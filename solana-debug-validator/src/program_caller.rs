
use std::{sync::{atomic::{AtomicU64, AtomicBool, Ordering}, Arc}, collections::HashMap};
use async_recursion::async_recursion;
use color_eyre::eyre;
use bokken_runtime::{ipc_comm::IPCComm, debug_env::{BokkenValidatorMessage, BokkenRuntimeMessage, BokkenAccountData, BorshAccountMeta}};
use solana_sdk::{pubkey::Pubkey, transaction::TransactionError, system_program, program_error::ProgramError};
use tokio::{net::UnixListener, task, sync::{Mutex, watch}};

use crate::{error::BokkenError, native_program_stubs::{NativeProgramStub, system_program::BokkenSystemProgram}};
#[derive(Debug)]
enum ProgramCallerExecStatus {
	Executed {
		return_code: u64,
		account_datas: HashMap<Pubkey, BokkenAccountData>
	},
	CPI {
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, BokkenAccountData>,
		call_depth: u8
	}
}

static COMM_NONCE: AtomicU64 = AtomicU64::new(0);
#[derive(Debug)]
pub struct ProgramCaller {
	native_programs: HashMap<Pubkey, Box<dyn NativeProgramStub>>,
	listener_handle: task::JoinHandle<eyre::Result<()>>,
	recieve_handle: task::JoinHandle<eyre::Result<()>>,
	should_stop: Arc<AtomicBool>,
	comms: Arc<Mutex<HashMap<Pubkey, IPCComm>>>,
	exec_notif: watch::Receiver<usize>,
	exec_logs: Arc<Mutex<HashMap<u64, Vec<String>>>>,
	exec_results: Arc<Mutex<HashMap<u64, ProgramCallerExecStatus>>>
}

impl ProgramCaller {
	pub fn new(
		listener: UnixListener,
	) -> Self {
		let should_stop = Arc::new(AtomicBool::new(false));
		let comms_mutex = Arc::new(Mutex::new(HashMap::new()));
		let exec_logs_mutex: Arc<Mutex<HashMap<u64, Vec<String>>>> = Arc::new(Mutex::new(HashMap::new()));
		let exec_results_mutex = Arc::new(Mutex::new(HashMap::new()));
		let (exec_notif_sender, exec_notif) = watch::channel(0usize);

		
		let should_stop_clone = should_stop.clone();
		let comms_mutex_clone = comms_mutex.clone();
		let listener_handle: task::JoinHandle<eyre::Result<()>> = task::spawn(async move {
			while !should_stop_clone.load(Ordering::Relaxed) {
				match listener.accept().await {
					Ok((stream, _addr)) => {
						let mut comms = comms_mutex_clone.lock().await;
						let (comm, program_id) = IPCComm::new_with_identifier::<Pubkey>(stream).await?;
						println!("Registered new debuggable program: {}", program_id);
						comms.insert(program_id, comm);
					}
					Err(_e) => { /* connection failed */ }
				}
			}
			Ok(())
		});
		let should_stop_clone = should_stop.clone();
		let comms_mutex_clone = comms_mutex.clone();
		let exec_logs_mutex_clone = exec_logs_mutex.clone();
		let exec_results_mutex_clone = exec_results_mutex.clone();
		let recieve_handle: task::JoinHandle<eyre::Result<()>> = task::spawn(async move {
			while !should_stop_clone.load(Ordering::Relaxed) {
				let mut stuff_executed = false;
				let mut comms = comms_mutex_clone.lock().await;
				for comm in comms.values_mut() {
					if let Some(msg) = comm.recv_msg::<BokkenRuntimeMessage>().await? {
						match msg {
							BokkenRuntimeMessage::Log { nonce, message } => {
								let mut exec_logs = exec_logs_mutex_clone.lock().await;
								if let Some(exec_log) = exec_logs.get_mut(&nonce) {
									exec_log.push(message);
								}
								// ignore for now
							},
							BokkenRuntimeMessage::Executed {
								nonce,
								return_code,
								account_datas
							} => {
								let mut exec_results = exec_results_mutex_clone.lock().await;
								exec_results.insert(
									nonce,
									ProgramCallerExecStatus::Executed {
										return_code,
										account_datas 
									}
								);
								stuff_executed = true;
							},
        					BokkenRuntimeMessage::CrossProgramInvoke {
								nonce,
								program_id,
								instruction,
								account_metas,
								account_datas,
								call_depth
							} => {
								let mut exec_results = exec_results_mutex_clone.lock().await;
								exec_results.insert(
									nonce,
									ProgramCallerExecStatus::CPI {
										program_id,
										instruction,
										account_metas,
										account_datas,
										call_depth 
									}
								);
								stuff_executed = true;
							},
						}
					}
				}
				drop(comms); // unlock it!
				if stuff_executed {
					exec_notif_sender.send_modify(|val| {
						(*val, _) = val.overflowing_add(1)
					})
				}// else{
				// 	sleep(Duration::from_millis(100)).await;
				// }
			}
			Ok(())
		});
		
		let mut native_programs = HashMap::new();
		native_programs.insert(
			system_program::id(),
			Box::new(BokkenSystemProgram::new()) as Box<dyn NativeProgramStub>
		);

		Self {
			native_programs,
			listener_handle,
			recieve_handle,
			should_stop,
			comms: comms_mutex,
			exec_logs: exec_logs_mutex,
			exec_results: exec_results_mutex,
			exec_notif
		}
	}
	pub async fn has_program_id(
		&self,
		program_id: &Pubkey
	) -> bool {
		self.native_programs.contains_key(program_id) || self.comms.lock().await.contains_key(program_id)
	}
	async fn wait_for_exec_status(
		&mut self,
		nonce: u64
	) -> Result<ProgramCallerExecStatus, BokkenError> {
		loop {
			if self.should_stop.load(Ordering::Relaxed) {
				return Err(BokkenError::Stopping);
			}
			{
				let mut exec_results = self.exec_results.lock().await;
				
				if let Some(status) = exec_results.remove(&nonce) {
					return Ok(status);
				}
				// exec_results gets dropped and unlocked
			}
			self.exec_notif.changed().await
				.map_err(|_|{BokkenError::ProgramClosedConnection})?;
		}
	}
	#[async_recursion]
	pub async fn call_program(
		&mut self,
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, BokkenAccountData>,
		call_depth: u8,
	) -> Result<(u64, Vec<String>, HashMap<Pubkey, BokkenAccountData>), BokkenError> {
		// Hashmap here?
		if let Some(native_program) = self.native_programs.get_mut(&program_id) {
			let mut account_datas = account_datas;
			native_program.clear_logs();
			native_program.logs_mut().push(format!("Program {} invoke [{}]", program_id, call_depth));
			match native_program.exec(instruction, account_metas, &mut account_datas) 	{
				Ok(_) => {
					native_program.logs_mut().push(format!("Program {} success", program_id));
					return Ok((0, native_program.logs().clone(), account_datas));
				},
				Err(err) => {
					native_program.logs_mut().push(format!("Program {} returned: {}", program_id, err));
					return Ok((err.into(), native_program.logs().clone(), account_datas));
				},
			}
		}
		let nonce = COMM_NONCE.fetch_add(1, Ordering::Relaxed);
		{
			let mut comms = self.comms.lock().await;
			let mut exec_logs = self.exec_logs.lock().await;
			comms.get_mut(&program_id)
				.ok_or(BokkenError::TransactionError(TransactionError::AccountNotFound))?
				.send_msg(
					BokkenValidatorMessage::Invoke {
						nonce,
						program_id,
						instruction,
						account_metas,
						account_datas,
						call_depth
					}
				).await?;
			exec_logs.insert(nonce, Vec::new());
			// comms and exec_logs get dropped and unlock
		}
		loop {
			if self.should_stop.load(Ordering::Relaxed) {
				return Err(BokkenError::Stopping);
			}
			match self.wait_for_exec_status(nonce).await? {
				ProgramCallerExecStatus::Executed {
					return_code,
					account_datas
				} => {
					let mut exec_logs = self.exec_logs.lock().await.remove(&nonce).unwrap_or_default();
						println!("TODO: Make sure lamports didn't get magically created or vanish");
						println!("TODO: Also make sure that the program only edited accounts that it has access to edit");
						println!("TODO: Maybe this could be done on the child process? (cuz CPI)");
					// This is a terrible hack
					exec_logs.insert(0, format!("Program {} invoke [{}]", program_id, call_depth));
					if return_code == 0 {
						exec_logs.push(format!("Program {} success", program_id));
					}else{
						exec_logs.push(format!("Program {} returned: {}", program_id, ProgramError::from(return_code)));
					}
					return Ok((return_code, exec_logs, account_datas));
				},
				ProgramCallerExecStatus::CPI {
					program_id: sub_program_id,
					instruction: sub_instruction,
					account_metas: sub_account_metas,
					account_datas: sub_account_datas,
					call_depth: sub_call_depth
				} => {
					let (sub_return_code, sub_logs, new_account_datas) = self.call_program(
						sub_program_id,
						sub_instruction,
						sub_account_metas,
						sub_account_datas,
						sub_call_depth + 1
					).await?;
					let mut exec_logs = self.exec_logs.lock().await;
					if let Some(exec_log) = exec_logs.get_mut(&nonce) {
						exec_log.extend(sub_logs);
					}
					let mut comms = self.comms.lock().await;
					comms.get_mut(&program_id)
						.ok_or(BokkenError::TransactionError(TransactionError::AccountNotFound))?
						.send_msg(
							BokkenValidatorMessage::CrossProgramInvokeResult {
								nonce,
								return_code: sub_return_code,
								account_datas: new_account_datas
							}
						).await?;
				},
			}
		}
	}
	pub fn stop(&self) {
		self.should_stop.store(true, Ordering::Relaxed);
	}
	pub async fn wait_until_stopped(self) -> eyre::Result<()> {
		self.recieve_handle.await??;
		self.listener_handle.await??;
		Ok(())
	}
}


use std::{sync::{atomic::{AtomicU64, AtomicBool, Ordering}, Arc}, collections::HashMap, time::Duration};
use color_eyre::eyre;
use solana_debug_runtime::{ipc_comm::IPCComm, debug_env::{DebugValidatorMessage, DebugRuntimeMessage, DebugAccountData, BorshAccountMeta}};
use solana_sdk::{pubkey::Pubkey, transaction::TransactionError};
use tokio::{net::UnixListener, task, sync::{Mutex, watch}, time::sleep};

use crate::error::DebugValidatorError;


static COMM_NONCE: AtomicU64 = AtomicU64::new(0);
#[derive(Debug)]
pub struct ProgramCaller {
	listener_handle: task::JoinHandle<eyre::Result<()>>,
	recieve_handle: task::JoinHandle<eyre::Result<()>>,
	should_stop: Arc<AtomicBool>,
	comms: Arc<Mutex<HashMap<Pubkey, IPCComm>>>,
	exec_notif: watch::Receiver<usize>,
	exec_logs: Arc<Mutex<HashMap<u64, Vec<String>>>>,
	exec_results: Arc<Mutex<HashMap<u64, (u64, HashMap<Pubkey, DebugAccountData>)>>>
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
					Err(e) => { /* connection failed */ }
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
					if let Some(msg) = comm.recv_msg::<DebugRuntimeMessage>().await? {
						match msg {
							DebugRuntimeMessage::Log { nonce, message } => {
								let mut exec_logs = exec_logs_mutex_clone.lock().await;
								if let Some(exec_log) = exec_logs.get_mut(&nonce) {
									exec_log.push(message);
								}
								// ignore for now
							},
							DebugRuntimeMessage::Executed {
								nonce,
								return_code,
								account_datas
							} => {
								let mut exec_results = exec_results_mutex_clone.lock().await;
								exec_results.insert(nonce, (return_code, account_datas));
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
		Self {
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
		self.comms.lock().await.contains_key(program_id)
	}
	pub async fn call_program(
		&mut self,
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, DebugAccountData>,
		call_depth: u8,
	) -> Result<(u64, Vec<String>, HashMap<Pubkey, DebugAccountData>), DebugValidatorError> {
		let nonce = COMM_NONCE.fetch_add(1, Ordering::Relaxed);
		{
			let mut comms = self.comms.lock().await;
			let mut exec_logs = self.exec_logs.lock().await;
			comms.get_mut(&program_id)
				.ok_or(DebugValidatorError::TransactionError(TransactionError::AccountNotFound))?
				.send_msg(
					DebugValidatorMessage::Invoke {
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
				return Err(DebugValidatorError::Stopping);
			}
			{
				let mut exec_results = self.exec_results.lock().await;
				if let Some((return_code, return_accounts)) = exec_results.remove(&nonce) {
					let mut exec_logs = self.exec_logs.lock().await;
					println!("TODO: Make sure lamports didn't get magically created or vanish");
					println!("TODO: Also make sure that the program only edited accounts that it has access to edit");
					println!("TODO: Maybe this could be done on the child process? (cuz CPI)");
					return Ok((return_code, exec_logs.remove(&nonce).unwrap_or_default(), return_accounts));
				}
				// exec_results gets dropped and unlocked
			}
			self.exec_notif.changed().await
				.map_err(|_|{DebugValidatorError::ProgramClosedConnection})?;
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

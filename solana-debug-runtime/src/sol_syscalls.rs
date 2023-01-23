use std::{sync::{Arc}, collections::{HashSet, HashMap}};

use solana_program::{program_stubs::SyscallStubs, program_error::{UNSUPPORTED_SYSVAR, ProgramError}, entrypoint::ProgramResult, pubkey::Pubkey, instruction::Instruction, account_info::AccountInfo};
use tokio::{sync::{Mutex, mpsc, RwLock}, task};
use itertools::Itertools;

use crate::{ipc_comm::IPCComm, debug_env::{BokkenRuntimeMessage, BokkenAccountData}, executor::{BokkenSolanaContext, execute_sol_program_thread, SolanaAccountsBlob}};

#[derive(Debug)]
pub(crate) enum BokkenSyscallMsg {
	PushContext {
		ctx: BokkenSolanaContext,
		msg_sender_clone: mpsc::Sender<BokkenSyscallMsg>,
	},
	PopContext
}

/// I'm making a big assumption that the paren't process isn't attempting to run this program in parralel.
/// But I do want to handle self-recursing calls
#[derive(Debug)]
pub(crate) struct BokkenSyscalls {
	ipc: Arc<Mutex<IPCComm>>,
	program_id: Pubkey,
	invoke_result_senders: Arc<Mutex<HashMap<u64, mpsc::Sender<(u64, HashMap<Pubkey, BokkenAccountData>)>>>>,
	// Using a mutex is just the easiest way to make the property mutable while being Send + Sync that I know of
	return_data: Arc<Mutex<Option<(Pubkey, Vec<u8>)>>>,
	contexts: Arc<Mutex<Vec<BokkenSolanaContext>>>,
}
impl BokkenSyscalls {
	pub fn new(
		ipc: Arc<Mutex<IPCComm>>,
		program_id: Pubkey,
		invoke_result_senders: Arc<Mutex<HashMap<u64, mpsc::Sender<(u64, HashMap<Pubkey, BokkenAccountData>)>>>>,
		mut msg_receiver: mpsc::Receiver<BokkenSyscallMsg>
	) -> Self {
		let contexts= Arc::new(Mutex::new(Vec::new()));
		let contexts_clone = contexts.clone();
		let ipc_clone = ipc.clone();
		task::spawn(async move {
			while let Some(msg) = msg_receiver.recv().await {
				match msg {
					BokkenSyscallMsg::PushContext { ctx, msg_sender_clone } => {
						let blob = ctx.blob.clone();
						let nonce = ctx.nonce();
						contexts_clone.lock().await.push(ctx);
						execute_sol_program_thread(nonce, blob, ipc_clone.clone(), msg_sender_clone).await;
					},
					BokkenSyscallMsg::PopContext => {
						contexts_clone.lock().await.pop();
					},
				}
			}
		});
		/* 
		thread::spawn(async move {
			while let Some(msg) = msg_receiver.recv().await {
				match msg {
					BokkenSyscallMsg::PushContext { ctx } => {
						context_values_clone.lock().await.push(ctx);
					},
					BokkenSyscallMsg::PopContext => {
						context_values_clone.lock().await.pop();
					},
				}
			}
		});
		*/
		Self {
			ipc,
			program_id,
			invoke_result_senders,
			return_data: Arc::new(Mutex::new(None)),
			contexts
		}
	}
	fn stack_height(&self) -> u8 {
		self.contexts.blocking_lock().last().expect("not be empty during program execution").cpi_height()
	}
	fn nonce(&self) -> u64 {
		self.contexts.blocking_lock().last().expect("not be empty during program execution").nonce()
	}
	fn account_data_lock(&self) -> Arc<RwLock<SolanaAccountsBlob>> {
		self.contexts.blocking_lock()
			.last()
			.expect("not be empty during program execution")
			.blob
			.clone()
	}
	fn is_valid_signer(&self, pubkey: &Pubkey) -> bool {
		self.contexts
			.blocking_lock()
			.last()
			.expect("not be empty during program execution")
			.is_signer(pubkey)
	}
	fn is_valid_writable(&self, pubkey: &Pubkey) -> bool {
		self.contexts
			.blocking_lock()
			.last()
			.expect("not be empty during program execution")
			.is_writable(pubkey)
	}
}

impl SyscallStubs for BokkenSyscalls {
	fn sol_log(&self, message: &str) {
		let msg = format!("Program logged: {}", message);
		println!("{}", msg);
		let mut ipc = self.ipc.blocking_lock();
		ipc.blocking_send_msg(
			BokkenRuntimeMessage::Log {
				nonce: self.nonce(),
				message: msg
			}
		).expect("Message encoding not to fail");
	}
	fn sol_log_compute_units(&self) {
		self.sol_log("WARNING: sol_log_compute_units() not available");
	}
	fn sol_invoke_signed(
		&self,
		instruction: &Instruction,
		account_infos: &[AccountInfo],
		signers_seeds: &[&[&[u8]]],
	) -> ProgramResult {
		let mut just_signed = HashSet::new();
		for signing_seed in signers_seeds.iter() {
			just_signed.insert(
				Pubkey::create_program_address(signing_seed, &self.program_id)?
			);
		}
		let mut outgoing_account_datas = HashMap::new();
		let ctx_account_data_lock = self.account_data_lock();
		{
			let ctx_acocunt_datas = ctx_account_data_lock.blocking_read();
			for (i, meta) in instruction.accounts.iter().enumerate() {
				if *account_infos[i].key != meta.pubkey {
					self.sol_log("Invoke: Accoune meta doesn't match account info");
					return Err(ProgramError::InvalidAccountData);
				}
				if meta.is_writable && !self.is_valid_writable(&meta.pubkey) {
					// TODO: Find out what error should be returned, or if this is even needed
					self.sol_log("Invoke: Cannot instruction requres an non-writable account to be writable");
					return Err(ProgramError::Custom(0));
				}
				if meta.is_signer && !self.is_valid_signer(&meta.pubkey) && !just_signed.contains(&meta.pubkey) {
					self.sol_log(format!(
						"Invoke: Account {} needs to be signed, but it isn't and doesn't match any given PDA seeds",
						meta.pubkey
					).as_str());
					return Err(ProgramError::MissingRequiredSignature);
				}
				outgoing_account_datas.insert(
					meta.pubkey.clone(),
					ctx_acocunt_datas.get_account_data(&meta.pubkey).expect("To have the account info we were just passed")
				);
			}
			// ctx_acocunt_datas drops unlocks
		}
		
		let mut receiver = {
			let (sender, receiver) = mpsc::channel(1);
			self.invoke_result_senders.blocking_lock().insert(self.nonce(), sender);
			receiver
			// self.invoke_result_senders unlocks
		};
		{
			self.ipc.blocking_lock().blocking_send_msg(
				BokkenRuntimeMessage::CrossProgramInvoke {
					nonce: self.nonce(),
					program_id: self.program_id,
					instruction: instruction.data.clone(),
					account_metas: instruction.accounts.iter().map(|v|{v.into()}).collect(),
					account_datas: HashMap::new(),
					call_depth: self.stack_height()
				}
			).expect("encoding to not fail");
			// self.ipc unlocks
		}
		let (return_code, account_datas) = receiver.blocking_recv().expect("get a response from CPI");
		{
			let mut ctx_acocunt_datas = ctx_account_data_lock.blocking_write();
			// We update these before potentially panicking for extra debugging flexibility
			for (pubkey, account_data) in account_datas.into_iter() {
				ctx_acocunt_datas.set_account_data(&pubkey, account_data)?;
			}
			// ctx_acocunt_datas drops and unlocks
		}
		if return_code != 0 {
			// SOL programs cannot catch a failed CPI, don't let 'em!
			panic!("CPI failed wirth return code {}", return_code);
		}
		Ok(())
	}
	fn sol_get_clock_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_epoch_schedule_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_fees_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_rent_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
		self.return_data.blocking_lock().clone()
	}
	fn sol_set_return_data(&self, data: &[u8]) {
		let mut return_data = self.return_data.blocking_lock();
		*return_data = Some((self.program_id, data.to_vec()));
	}
	fn sol_log_data(&self, fields: &[&[u8]]) {
		self.sol_log(format!("data: {}", fields.iter().map(base64::encode).join(" ")).as_str());
	}
	fn sol_get_processed_sibling_instruction(&self, _index: usize) -> Option<Instruction> {
		self.sol_log("WARNING: sol_get_processed_sibling_instruction() not available");
		None
	}
	fn sol_get_stack_height(&self) -> u64 {
		self.stack_height() as u64
	}
}

use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, collections::HashSet};

use solana_program::{program_stubs::SyscallStubs, program_error::{UNSUPPORTED_SYSVAR, ProgramError}, entrypoint::ProgramResult, pubkey::Pubkey, instruction::Instruction, account_info::AccountInfo};
use tokio::sync::{Mutex, mpsc};
use itertools::Itertools;

use crate::{ipc_comm::IPCComm, debug_env::DebugRuntimeMessage};

#[derive(Debug)]
pub(crate) struct DebugValidatorSyscallContext {
	pub nonce: u64,
	pub stack_height: u8,
	pub valid_writables: HashSet<Pubkey>,
	pub valid_signers: HashSet<Pubkey>
}
#[derive(Debug)]
pub(crate) enum DebugValidatorSyscallMsg {
	PushContext {
		ctx: DebugValidatorSyscallContext
	},
	PopContext
}

/// I'm making a big assumption that the paren't process isn't attempting to run this program in parralel.
/// But I do want to handle self-recursing calls
#[derive(Debug)]
pub(crate) struct DebugValidatorSyscalls {
	// I have to use blocking mutexes 🙃
	ipc: Arc<Mutex<IPCComm>>,
	program_id: Pubkey,
	// Using a mutex is just the easiest way to make the property mutable while being Send + Sync that I know of
	return_data: Arc<Mutex<Option<(Pubkey, Vec<u8>)>>>,
	context_values: Arc<Mutex<Vec<DebugValidatorSyscallContext>>>,
}
impl DebugValidatorSyscalls {
	pub fn new(
		ipc: Arc<Mutex<IPCComm>>,
		program_id: Pubkey,
		mut msg_receiver: mpsc::Receiver<DebugValidatorSyscallMsg>
	) -> Self {
		let context_values= Arc::new(Mutex::new(Vec::new()));
		let context_values_clone = context_values.clone();
		tokio::task::spawn(async move {
			while let Some(msg) = msg_receiver.recv().await {
				match msg {
					DebugValidatorSyscallMsg::PushContext { ctx } => {
						context_values_clone.lock().await.push(ctx);
					},
					DebugValidatorSyscallMsg::PopContext => {
						context_values_clone.lock().await.pop();
					},
				}
			}
		});
		Self {
			ipc,
			program_id,
			return_data: Arc::new(Mutex::new(None)),
			context_values
		}
	}
	fn nonce(&self) -> u64 {
		self.context_values.blocking_lock().last().expect("not be empty during program execution").nonce
	}
	fn stack_height(&self) -> u8 {
		self.context_values.blocking_lock().last().expect("not be empty during program execution").stack_height
	}
	fn is_valid_signer(&self, pubkey: &Pubkey) -> bool {
		self.context_values
			.blocking_lock()
			.last()
			.expect("not be empty during program execution")
			.valid_signers
			.contains(pubkey)
	}
	fn is_valid_writable(&self, pubkey: &Pubkey) -> bool {
		self.context_values
			.blocking_lock()
			.last()
			.expect("not be empty during program execution")
			.valid_writables
			.contains(pubkey)
	}
}

impl SyscallStubs for DebugValidatorSyscalls {
	fn sol_log(&self, message: &str) {
		let msg = format!("Program logged: {}", message);
		println!("{}", msg);
		let mut ipc = self.ipc.blocking_lock();
		ipc.blocking_send_msg(
			DebugRuntimeMessage::Log {
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
		for meta in instruction.accounts.iter() {
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
			}
		}
		panic!("TODO: sol_invoke_signed");
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

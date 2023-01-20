use std::{mem::size_of, collections::HashMap, sync::Arc, thread};

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Zeroable, Pod};
use solana_program::{
	entrypoint::MAX_PERMITTED_DATA_INCREASE,
	pubkey::Pubkey,
	program_error::ProgramError, instruction::AccountMeta
};
use tokio::{sync::{Mutex, RwLock, mpsc}, task};

use crate::{debug_env::{DebugAccountData, DebugRuntimeMessage}, ipc_comm::IPCComm, sol_syscalls::DebugValidatorSyscallMsg};
// use lazy_static::lazy_static;



/// Maximum number of bytes a program may add to an account during a single realloc
//pub const MAX_PERMITTED_DATA_INCREASE: usize = 1_024 * 10;


#[derive(PartialEq, Eq, Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(crate) struct AccountInfoHeader {
	_0xff: u8,
	is_signer: u8, // bool
	is_writable: u8, // bool
	executable: u8, // bool
	pub original_data_len: u32,
	pub pubkey: Pubkey,
	pub owner: Pubkey,
	pub lamports: u64,
	pub data_len: u64
}
impl AccountInfoHeader {
	pub fn is_signer(&self) -> bool {
		self.is_signer > 0
	}
	pub fn is_writable(&self) -> bool {
		self.is_writable > 0
	}
	pub fn executable(&self) -> bool {
		self.executable > 0
	}
}


#[derive(Debug)]
pub(crate) struct SolanaAccountsBlob {
	pub account_offsets: HashMap<Pubkey, usize>,
	pub bytes: Vec<u8>
}
impl SolanaAccountsBlob {
	pub fn new(
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<AccountMeta>,
		mut account_datas: HashMap<Pubkey, DebugAccountData>
	) -> Self {
		let mut blob: Vec<u8> = Vec::with_capacity(
			account_metas.len() * 20480 + // this value is arbitrary
			size_of::<u64>() + 
			instruction.len() +
			size_of::<Pubkey>()
		);
		blob.extend((account_metas.len() as u64).to_le_bytes());

		let mut account_indices: HashMap<Pubkey, usize> = HashMap::new();
		let mut account_offsets: HashMap<Pubkey, usize> = HashMap::new();
		for (index, account_meta) in account_metas.iter().enumerate() {
			if let Some(entry_index) = account_indices.get(&account_meta.pubkey) {
				blob.extend((*entry_index as u64).to_le_bytes());
			}else{
				let account_data = account_datas.remove(&account_meta.pubkey)
					.expect("The account metas should reference accounts in the account datas");
				account_indices.insert(account_meta.pubkey, index);
				account_offsets.insert(account_meta.pubkey, blob.len());

				blob.push(u8::MAX);
				blob.push(account_meta.is_signer as u8);
				blob.push(account_meta.is_writable as u8);
				blob.push(account_data.executable as u8);
				blob.extend((account_data.data.len() as u32).to_le_bytes()); // "Original data length" (immediatly overwritten?)
				blob.extend(account_meta.pubkey.as_ref());
				blob.extend(account_data.owner.as_ref());
				blob.extend((account_data.lamports).to_le_bytes());
				blob.extend((account_data.data.len() as u64).to_le_bytes());
				blob.extend(account_data.data);
				blob.extend(vec![0; MAX_PERMITTED_DATA_INCREASE]);
				blob.extend(vec![0; blob.len() % 8]);
				blob.extend(account_data.rent_epoch.to_le_bytes());		
			}
		}
		blob.extend((instruction.len() as u64).to_le_bytes());
		blob.extend(instruction);
		blob.extend(program_id.as_ref());
		Self {
			bytes: blob,
			account_offsets
		}
	}
	pub fn get_account_data(&self, pubkey: &Pubkey) -> Option<DebugAccountData> {
		if let Some(account_offset) = self.account_offsets.get(pubkey) {
			let account_data_offset = *account_offset + std::mem::size_of::<AccountInfoHeader>();
			let account_header = bytemuck::from_bytes::<AccountInfoHeader>(
				&self.bytes[*account_offset..account_data_offset]
			);
			let rent_epoch_offset =
				account_data_offset +
				account_header.original_data_len as usize +
				MAX_PERMITTED_DATA_INCREASE +
				(account_header.original_data_len as usize % 8);
			
			Some( DebugAccountData {
				lamports: account_header.lamports,
				data: self.bytes[account_data_offset..{account_data_offset + account_header.data_len as usize}].to_vec(),
				owner: account_header.owner,
				executable: account_header.executable > 0,
				rent_epoch: u64::from_le_bytes(self.bytes[rent_epoch_offset..{rent_epoch_offset + 8}].try_into().unwrap())
			})
		}else{
			None
		}
	}
	pub fn set_account_data(&mut self, pubkey: &Pubkey, account_data: DebugAccountData) -> Result<(), ProgramError> {
		if let Some(account_offset) = self.account_offsets.get(pubkey) {
			let account_data_offset = *account_offset + std::mem::size_of::<AccountInfoHeader>();
			let account_header = bytemuck::from_bytes_mut::<AccountInfoHeader>(
				&mut self.bytes[*account_offset..account_data_offset]
			);
			if account_data.data.len() > account_header.original_data_len as usize + MAX_PERMITTED_DATA_INCREASE {
				println!("Debug runtime: set_account_data: {} was grown too much", pubkey);
				return Err(ProgramError::InvalidRealloc);
			}
			account_header.data_len = account_data.data.len() as u64;
			account_header.lamports = account_data.lamports;
			account_header.owner = account_data.owner;
			self.bytes[account_data_offset..{account_data_offset + account_data.data.len()}].copy_from_slice(&account_data.data);
			Ok(())
		}else{
			println!(
				"Debug runtime: set_account_data called with {} but we have no idea what that account is",
				pubkey
			);
			Err(ProgramError::UninitializedAccount)
		}
	}
	pub fn get_account_data_header(&self, pubkey: &Pubkey) -> Option<&AccountInfoHeader> {
		if let Some(account_offset) = self.account_offsets.get(pubkey) {
			let account_data_offset = *account_offset + std::mem::size_of::<AccountInfoHeader>();
			let account_header = bytemuck::from_bytes::<AccountInfoHeader>(
				&self.bytes[*account_offset..account_data_offset]
			);
			Some(account_header)
		}else{
			None
		}
	}
	pub fn is_writable(&self, pubkey: &Pubkey) -> bool {
		if let Some(account_header) = self.get_account_data_header(pubkey) {
			account_header.is_writable() && !account_header.executable()
		}else{
			false
		}
	}
	pub fn is_signer(&self, pubkey: &Pubkey) -> bool {
		if let Some(account_header) = self.get_account_data_header(pubkey) {
			account_header.is_signer()
		}else{
			false
		}
	}

	pub fn get_account_datas(&self) -> HashMap<Pubkey, DebugAccountData> {
		let mut result = HashMap::new();
		for pubkey in self.account_offsets.keys() {
			result.insert(
				*pubkey,
				self.get_account_data(pubkey).expect("the value of the keys we are iterating over")
			);
		}
		result
	}
}


#[derive(Debug)]
pub(crate) struct SolanaDebugContext {
	// executed: bool,
	pub blob: Arc<RwLock<SolanaAccountsBlob>>,
	nonce: u64,
	cpi_height: u8
}
impl SolanaDebugContext {
	pub fn new(
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<AccountMeta>,
		account_datas: HashMap<Pubkey, DebugAccountData>,
		nonce: u64,
		cpi_height: u8,
	) -> Self {
		
		Self {
			// executed: false,
			blob: Arc::new(RwLock::new(
				SolanaAccountsBlob::new(
					program_id,
					instruction,
					account_metas,
					account_datas
				)
			)),
			nonce,
			cpi_height
		}
	}
	pub fn get_account_data(&self, pubkey: &Pubkey) -> Option<DebugAccountData> {
		self.blob.blocking_read().get_account_data(pubkey)
	}
	pub fn is_writable(&self, pubkey: &Pubkey) -> bool {
		self.blob.blocking_read().is_writable(pubkey)
	}
	pub fn is_signer(&self, pubkey: &Pubkey) -> bool {
		self.blob.blocking_read().is_signer(pubkey)
	}

	pub fn get_account_datas(&self) -> HashMap<Pubkey, DebugAccountData> {
		self.blob.blocking_read().get_account_datas()
	}
	pub fn cpi_height(&self) -> u8 {
		self.cpi_height
	}
	pub fn nonce(&self) -> u64 {
		self.nonce
	}
}
/// Spawns a new thread and runs entrypoint in that thread
/// Does not await until the new thread is finished, await is only used to properly use the RwLock
pub(crate) async fn execute_sol_program_thread(
	nonce: u64,
	blob: Arc<RwLock<SolanaAccountsBlob>>,
	comm: Arc<Mutex<IPCComm>>,
	context_drop_notifier: mpsc::Sender<DebugValidatorSyscallMsg>
) {
		// This is "unsafe", but we cannot write-lock the blob during the entire SOL program's execution.
		// This is because we need to update the account data as a result of a CPI. If we locked it here, then we'd
		// deadlock ourselves as we'd never be able to update the account data.
		let blob_ptr = {
			// And so, we're bypassing the RwLock to make that happen.
			blob.read().await.bytes.as_ptr() as usize
		};
		// spawning a thread is used cuz invoke is a blocking method

		thread::spawn(move || {
			// A thread to watch a thread that might panic
			let result = thread::spawn(move || {
				println!("DEBUG: execute_sol_program_thread: inner thread started");
				extern "C" {
					// Yep, that's it. We just statically link with whatever function is called "entrypoint"
					fn entrypoint(input: *mut u8) -> u64;
				}
				let result = unsafe {
					entrypoint(blob_ptr as *mut u8)
				};
				println!("DEBUG: execute_sol_program_thread: inner thread finished");
				result
			}).join();
			println!("DEBUG: execute_sol_program_thread: Finished entrypoint call");
			let mut comm = comm.blocking_lock();
			println!("DEBUG: execute_sol_program_thread: locked comms");
			context_drop_notifier.blocking_send(
				DebugValidatorSyscallMsg::PopContext
			).expect("mpsc::Sender to not fail");
			println!("DEBUG: execute_sol_program_thread: sent pop context");
			let account_datas = blob.blocking_read().get_account_datas();
			match result {
				Ok(return_code) => {
					comm.blocking_send_msg(
						DebugRuntimeMessage::Executed{
							nonce,
							return_code,
							account_datas
						}
					).expect("encoding to not fail");
				},
				Err(err) => {
					let panic_msg = match err.downcast_ref::<&str>() {
						Some(str) => str.to_string(),
						None => {
							match err.downcast_ref::<String>() {
								Some(str) => str.clone(),
								None => String::from("<Unknown panic message>")
							}
						},
					};
					comm.blocking_send_msg(
						DebugRuntimeMessage::Log{
							nonce,
							message: format!("Program panicked: {}", panic_msg)
						}
					).expect("encoding to not fail");
					comm.blocking_send_msg(
						// TODO: Treat panics differently
						DebugRuntimeMessage::Executed{
							nonce,
							return_code: ProgramError::Custom(0).into(),
							account_datas
						}
					).expect("encoding to not fail");
				},
			}
			println!("DEBUG: execute_sol_program_thread: send result over comms");
		});
}

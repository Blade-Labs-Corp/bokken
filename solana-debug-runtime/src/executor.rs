use std::{mem::size_of, collections::HashMap, sync::Arc};

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Zeroable, Pod};
use solana_program::{
	entrypoint::MAX_PERMITTED_DATA_INCREASE,
	pubkey::Pubkey,
	program_error::ProgramError, instruction::AccountMeta
};

use crate::debug_env::DebugAccountData;
// use lazy_static::lazy_static;



/// Maximum number of bytes a program may add to an account during a single realloc
//pub const MAX_PERMITTED_DATA_INCREASE: usize = 1_024 * 10;


#[derive(PartialEq, Eq, Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct RawAccountInfoHeader {
	_0xff: u8,
	pub is_signer: u8, // bool
	pub is_writable: u8, // bool
	pub executable: u8, // bool
	pub original_data_len: u32,
	pub pubkey: Pubkey,
	pub owner: Pubkey,
	pub lamports: u64,
	pub data_len: u64
}
pub struct SolanaDebugContext {
	executed: bool,
	blob: Vec<u8>,
	account_offsets: HashMap<Pubkey, usize>,
	call_depth: u8
}
impl SolanaDebugContext {
	pub fn new(
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<AccountMeta>,
		mut account_datas: HashMap<Pubkey, DebugAccountData>,
		call_depth: u8,
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
			executed: false,
			blob,
			account_offsets,
			call_depth
		}
	}
	pub fn get_account_data(&self, pubkey: &Pubkey) -> Option<DebugAccountData> {
		if let Some(account_offset) = self.account_offsets.get(pubkey) {
			let account_data_offset = *account_offset + std::mem::size_of::<RawAccountInfoHeader>();
			let account_header = bytemuck::from_bytes::<RawAccountInfoHeader>(
				&self.blob[*account_offset..account_data_offset]
			);
			let rent_epoch_offset =
				account_data_offset +
				account_header.original_data_len as usize +
				MAX_PERMITTED_DATA_INCREASE +
				(account_header.original_data_len as usize % 8);
			
			Some( DebugAccountData {
				lamports: account_header.lamports,
				data: self.blob[account_data_offset..{account_data_offset + account_header.data_len as usize}].to_vec(),
				owner: account_header.owner,
				executable: account_header.executable > 0,
				rent_epoch: u64::from_le_bytes(self.blob[rent_epoch_offset..{rent_epoch_offset + 8}].try_into().unwrap())
			})
		}else{
			None
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
	pub async fn execute_sol_program(&mut self) -> u64 {
		if self.executed {
			panic!("SolanaDebugContext should only run once")
		}
		self.executed = true;

		// This is dumb, but I couldn't quickly find a better way
		let mut blob_clone = self.blob.clone();
		// spawn_blocking is used cuz invoke is a blocking method
		let handle = tokio::task::spawn_blocking(move || {
			extern "C" {
				// Yep, that's it. We just statically link with whatever function is called "entrypoint"
				fn entrypoint(input: *mut u8) -> u64;
			}
			let return_code = unsafe {
				entrypoint(blob_clone.as_mut_ptr())
			};
			(return_code, blob_clone)
		});
		let (return_code, modified_blob) = handle.await.unwrap();

		// Ugh
		self.blob = modified_blob;
		return_code
	}
}

use std::{path::PathBuf, collections::{HashMap, HashSet}, io};

use borsh::{BorshSerialize, BorshDeserialize};
use color_eyre::eyre;
use bokken_runtime::debug_env::{BokkenAccountData, BorshAccountMeta};
use solana_sdk::{pubkey, pubkey::Pubkey, system_program, transaction::TransactionError};
use tokio::fs;
use lazy_static::lazy_static;

use crate::{error::BokkenError, program_caller::ProgramCaller};

const RENT_BASE_SIZE: u64 = 128;
pub const PUBKEY_NULL: Pubkey = pubkey!("nu11111111111111111111111111111111111111111");
pub const PUBKEY_DEBUG_PROGRAM_LOADER: Pubkey = pubkey!("Debugab1eProgramLoader111111111111111111111");
lazy_static! {
    static ref GHOST_DATA: Vec<u8> = vec![0xf0, 0x9f, 0x91, 0xbb, 0xf0, 0x9f, 0x90, 0x9b, 0xf0, 0x9f, 0xa7, 0x91, 0xe2, 0x80, 0x8d, 0xf0, 0x9f, 0x92, 0xbb];
}

#[derive(Debug)]
pub struct BokkenLedgerInitConfig {
	pub initial_mint: Pubkey,
	pub initial_mint_lamports: u64
}

/// Abstraction around Bokken's save directory
#[derive(Debug)]
pub struct BokkenLedger {
	base_path: PathBuf,
	accounts_path: PathBuf,
	program_caller: ProgramCaller,
	state: BokkenLedgerState
}
#[derive(Debug)]
pub struct BokkenLedgerInstruction {
	pub program_id: Pubkey,
	pub account_metas: Vec<BorshAccountMeta>,
	pub data: Vec<u8>
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BokkenLedgerAccountReturnChoice {
	None,
	All,
	Edited,
	Only(Vec<Pubkey>)
}
impl BokkenLedger {
	/// Manages Bokken's state at the specified path
	/// 
	pub async fn new(
		base_path: PathBuf,
		program_caller: ProgramCaller,
		init_mint_config: Option<BokkenLedgerInitConfig>
	) -> eyre::Result<Self> {
		let accounts_path = {
			let mut p = base_path.clone();
			p.push("accounts");
			p
		};
		let state_path = {
			let mut p = base_path.clone();
			p.push("state.blob");
			p
		};
		let mut new_self = Self {
			base_path,
			accounts_path,
			program_caller,
			state: BokkenLedgerState::new(state_path).await?
		};
		match fs::create_dir(&new_self.base_path).await {
			Ok(_) => {
				fs::create_dir(&new_self.accounts_path).await?;
				let init_mint_config = init_mint_config.ok_or(BokkenError::InitConfigIsNone)?;
				let init_mint_account = BokkenAccountData {
					lamports: init_mint_config.initial_mint_lamports,
					data: Vec::new(),
					owner: system_program::id(),
					executable: false,
					rent_epoch: 0
				};
				new_self.save_account(&init_mint_config.initial_mint, &init_mint_account).await?;
				new_self.state.inc_slot().await?;
			},
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				// TODO: Verify integrity?
			},
			Err(e) => {
				return Err(e.into())
			}
		}
		Ok(new_self)
	}
	pub fn slot(&self) -> u64 {
		self.state.slot()
	}
	pub fn blockhash(&self) -> [u8; 32] {
		// We're not actually doing anything here yet, pass a fake value so things work
		let mut result = <[u8; 32]>::default();
		result[0..8].copy_from_slice(&self.slot().to_le_bytes());
		result
	}
	pub fn calc_min_balance_for_rent_exemption(&self, data_len: u64) -> u64 {
		(RENT_BASE_SIZE + data_len) * self.state.rent_per_byte_year() * 2
	}
	pub async fn save_account(&self, pubkey: &Pubkey, data: &BokkenAccountData) -> Result<(), BokkenError> {
		let mut account_path = self.accounts_path.clone();
		account_path.push(pubkey.to_string());
		fs::create_dir_all(&account_path).await?;
		account_path.push(self.slot().to_string());
		fs::write(
			&account_path,
			if data.lamports == 0 {
				BokkenAccountData::default().try_to_vec()?
			}else{
				data.try_to_vec()?
			}
		).await?;
		Ok(())
	}
	pub async fn read_account(&self, pubkey: &Pubkey) -> Result<BokkenAccountData, BokkenError> {
		if self.program_caller.has_program_id(pubkey).await {
			return Ok(
				BokkenAccountData {
					lamports: 0xf09f91bb,
					data: GHOST_DATA.clone(),
					owner: PUBKEY_DEBUG_PROGRAM_LOADER,
					executable: true,
					rent_epoch: 0
				}
			)
		}
		let mut account_path = self.accounts_path.clone();
		account_path.push(pubkey.to_string());
		
		match fs::read_dir(&account_path).await {
			Ok(mut files) => {
				let mut max_slot = 0u64;
				while let Some(file) = files.next_entry().await? {
					let slot = file.file_name().to_str().unwrap_or_default().parse::<u64>().unwrap_or_default();
					if slot > max_slot {
						max_slot = slot;
					}
				}
				account_path.push(max_slot.to_string());
				match fs::read(account_path).await {
					Ok(file_data) => {
						let file_data_parsed = BokkenAccountData::try_from_slice(&file_data)?;
						Ok(file_data_parsed)
					},
					Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
						Ok(BokkenAccountData::default())
					},
					Err(e) => {
						return Err(e.into())
					}
				}
			},
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				Ok(BokkenAccountData::default())
			},
			Err(e) => {
				return Err(e.into())
			}
		}
	}
	async fn execute_instruction(
		&mut self,
		instruction: BokkenLedgerInstruction,
		call_depth: u8,
		state: &mut HashMap<Pubkey, BokkenAccountData>
	) -> Result<(u64, Vec<String>), BokkenError> {
		// Only send ixs required to the child process (this probably wastes more perf than it saves)
		let account_datas_for_ix = {
		 	let mut account_datas_for_ix = HashMap::new();
			for meta in instruction.account_metas.iter() {
				if !account_datas_for_ix.contains_key(&meta.pubkey) {
					account_datas_for_ix.insert(
						meta.pubkey.clone(),
						state.remove(&meta.pubkey).ok_or(TransactionError::AccountNotFound)?
					);
				}
			}
			account_datas_for_ix
		};

		let (return_code, logs, account_datas_for_ix) = self.program_caller.call_program(
			instruction.program_id,
			instruction.data,
			instruction.account_metas,
			account_datas_for_ix,
			call_depth
		).await?;

		// do stuff
		for (pubkey, account_data) in account_datas_for_ix.into_iter() {
			// re-insert edited state back in
			state.insert(pubkey, account_data);
		}
		Ok((return_code, logs))
	}
	/// Execute the specified data as a transaction instruction
	/// Saves any changes and increments the block slot if `commit_changes` is true
	pub async fn execute_instructions(
		&mut self,
		fee_payer: &Pubkey,
		instructions: Vec<BokkenLedgerInstruction>,
		return_choice: BokkenLedgerAccountReturnChoice,
		commit_changes: bool
	) -> Result<(HashMap<Pubkey, BokkenAccountData>, Vec<String>), BokkenError> {
		let mut the_big_log = Vec::new();
		let mut unique_sigs = HashSet::new();
		unique_sigs.insert(fee_payer.clone());
		let account_datas = {
			let mut account_datas = HashMap::new();
			account_datas.insert(fee_payer.clone(), self.read_account(fee_payer).await?);
			for ix in instructions.iter() {
				for meta in ix.account_metas.iter() {
					if meta.is_signer {
						unique_sigs.insert(meta.pubkey.clone());
					}
					if !account_datas.contains_key(&meta.pubkey) {
						account_datas.insert(meta.pubkey, self.read_account(&meta.pubkey).await?);
					}
				}
			}
			account_datas
		};
		let mut account_datas_changed = account_datas.clone();
		{
			// Take the fee away!
			let fee_payer = account_datas_changed.get_mut(fee_payer)
				.expect("For the fee payer data to be where we put it");
			// sig fee is hard-coded for now
			// TODO: care about about the 128 bytes for rent
			fee_payer.lamports = fee_payer.lamports.checked_sub(
				5000 * unique_sigs.len() as u64
			).ok_or(TransactionError::InsufficientFundsForFee)?;
			// fee_payer gets dropped
		}

		for (i, ix) in instructions.into_iter().enumerate() {
			let (return_code, logs) = self.execute_instruction(ix, 1, &mut account_datas_changed).await?;
			the_big_log.extend(logs);
			if return_code != 0 {
				return Err(BokkenError::InstructionExecError(i, return_code.into(), the_big_log));
			}
		}
		let edited_accounts = {
			let mut result = HashMap::new();
			for (pubkey, old_data) in account_datas.into_iter() {
				let new_data = account_datas_changed.get(&pubkey).unwrap().clone();
				if new_data != old_data {
					if commit_changes {
						self.save_account(&pubkey, &new_data).await?;
					}
					result.insert(pubkey, new_data);
				}
			}
			result
		};
		let account_data_result = match return_choice {
			BokkenLedgerAccountReturnChoice::None => {
				HashMap::new()
			}
			BokkenLedgerAccountReturnChoice::All => {
				account_datas_changed
			},
			BokkenLedgerAccountReturnChoice::Edited => {
				edited_accounts
			},
			BokkenLedgerAccountReturnChoice::Only(pubkeys) => {
				let mut result = HashMap::new();
				for pubkey in pubkeys.into_iter() {
					result.insert(pubkey, account_datas_changed.get(&pubkey).unwrap().clone());
				}
				result
			}
		};
		if commit_changes {
			self.state.inc_slot().await?;
			println!("TODO: Save log and ix history");
		}
		Ok((account_data_result, the_big_log))
	}
}

/// Global state for the Bokken ledger
#[derive(Debug, Default, BorshSerialize, BorshDeserialize)]
struct BokkenLedgerState {
	#[borsh_skip]
	path: PathBuf,
	slot: u64,
	rent_per_byte_year: u64,

}
impl BokkenLedgerState {
	pub async fn new(path: PathBuf) -> Result<Self, io::Error> {
		match fs::read(&path).await {
			Ok(data) => {
				let mut new_self = Self::try_from_slice(&data)?;
				new_self.path = path;
				Ok(new_self)
			},
			Err(err) if err.kind() == io::ErrorKind::NotFound => {
				Ok(
					Self {
						path,
						slot: 0,
						rent_per_byte_year: 348
					}
				)
			},
			Err(err) => Err(err),
		}
	}
	pub async fn save(&self) -> Result<(), io::Error> {
		fs::write(&self.path, self.try_to_vec()?).await
	}
	pub async fn inc_slot(&mut self) -> Result<(), io::Error> {
		self.slot += 1;
		println!("BokkenLedgerState: inc_slot to {}", self.slot);
		self.save().await
	}
	pub fn slot(&self) -> u64 {
		self.slot
	}
	pub fn rent_per_byte_year(&self) -> u64 {
		self.rent_per_byte_year
	}
}

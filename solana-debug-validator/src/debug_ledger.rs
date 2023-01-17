use std::{path::PathBuf, collections::HashMap, io};

use borsh::{BorshSerialize, BorshDeserialize};
use solana_debug_runtime::debug_env::{DebugAccountData, BorshAccountMeta};
use solana_sdk::{pubkey, pubkey::Pubkey, system_program, program_error::ProgramError, transaction::TransactionError};
use tokio::fs;
use lazy_static::lazy_static;

use crate::{error::DebugValidatorError, program_caller::ProgramCaller};

pub const PUBKEY_NULL: Pubkey = pubkey!("nu11111111111111111111111111111111111111111");
pub const PUBKEY_DEBUG_PROGRAM_LOADER: Pubkey = pubkey!("Debugab1eProgramLoader111111111111111111111");
lazy_static! {
    static ref GHOST_DATA: Vec<u8> = vec![0xf0, 0x9f, 0x91, 0xbb, 0xf0, 0x9f, 0x90, 0x9b, 0xf0, 0x9f, 0xa7, 0x91, 0xe2, 0x80, 0x8d, 0xf0, 0x9f, 0x92, 0xbb];
}

#[derive(Debug)]
pub struct DebugLedgerInitConfig {
	pub initial_mint: Pubkey,
	pub initial_mint_lamports: u64
}
#[derive(Debug)]
pub struct DebugLedger {
	base_path: PathBuf,
	accounts_path: PathBuf,
	program_caller: ProgramCaller,
	state: DebugLedgerState
}
#[derive(Debug)]
pub struct DebugLedgerInstruction {
	pub program_id: Pubkey,
	pub account_metas: Vec<BorshAccountMeta>,
	pub data: Vec<u8>
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugLedgerAccountReturnChoice {
	All,
	Edited,
	Only(Vec<Pubkey>)
}
impl DebugLedger {
	pub async fn new(
		base_path: PathBuf,
		program_caller: ProgramCaller,
		init_mint_config: Option<DebugLedgerInitConfig>
	) -> Result<Self, DebugValidatorError> {
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
		let sayulf = Self {
			base_path,
			accounts_path,
			program_caller,
			state: DebugLedgerState::new(state_path).await?
		};
		match fs::create_dir(&sayulf.base_path).await {
			Ok(_) => {
				fs::create_dir(&sayulf.accounts_path).await?;
				let init_mint_config = init_mint_config.ok_or(DebugValidatorError::InitConfigIsNone)?;
				let init_mint_account = DebugAccountData {
					lamports: init_mint_config.initial_mint_lamports,
					data: Vec::new(),
					owner: system_program::id(),
					executable: false,
					rent_epoch: 0
				};
				sayulf.save_account(&init_mint_config.initial_mint, &init_mint_account).await?;

				// TODO: Don't create this account when we get the system program running
				sayulf.save_account(
					&pubkey!("TheDebugab1eProgramTestState111111111111111"),
					&DebugAccountData {
						lamports: 1000000000,
						data: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
						owner: pubkey!("TheDebugab1eProgram111111111111111111111111"),
						executable: false,
						rent_epoch: 0
					}
				).await?;
			},
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				// TODO: Verify integrity?
			},
			Err(e) => {
				return Err(e.into())
			}
		}
		Ok(sayulf)
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

	pub async fn save_account(&self, pubkey: &Pubkey, data: &DebugAccountData) -> Result<(), DebugValidatorError> {
		let account_path = {
			let mut p = self.accounts_path.clone();
			p.push(pubkey.to_string());
			p
		};
		fs::write(
			&account_path,
			data.try_to_vec()?
		).await?;
		Ok(())
	}
	pub async fn read_account(&self, pubkey: &Pubkey) -> Result<DebugAccountData, DebugValidatorError> {
		if self.program_caller.has_program_id(pubkey).await {
			return Ok(
				DebugAccountData {
					lamports: 0xf09f91bb,
					data: GHOST_DATA.clone(),
					owner: PUBKEY_DEBUG_PROGRAM_LOADER,
					executable: true,
					rent_epoch: 0
				}
			)
		}
		let account_path = {
			let mut p = self.accounts_path.clone();
			p.push(pubkey.to_string());
			p
		};
		let file_data = fs::read(account_path).await?;
		let file_data_parsed = DebugAccountData::try_from_slice(&file_data)?;
		Ok(file_data_parsed)
	}
	pub async fn execute_instruction(
		&mut self,
		instruction: DebugLedgerInstruction,
		call_depth: u8,
		state: &mut HashMap<Pubkey, DebugAccountData>
	) -> Result<((u64, Vec<String>)), DebugValidatorError> {
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
	pub async fn execute_instructions(
		&mut self,
		instructions: Vec<DebugLedgerInstruction>,
		return_choice: DebugLedgerAccountReturnChoice
	) -> Result<(HashMap<Pubkey, DebugAccountData>, Vec<String>), DebugValidatorError> {
		let mut the_big_log = Vec::new();
		let account_datas = {
			let mut account_datas = HashMap::new();
			for ix in instructions.iter() {
				for meta in ix.account_metas.iter() {
					if !account_datas.contains_key(&meta.pubkey) {
						account_datas.insert(meta.pubkey, self.read_account(&meta.pubkey).await?);
					}
				}
			}
			account_datas
		};
		let mut account_datas_changed = account_datas.clone();
		for (i, ix) in instructions.into_iter().enumerate() {
			let (return_code, logs) = self.execute_instruction(ix, 1, &mut account_datas_changed).await?;
			the_big_log.extend(logs);
			if return_code != 0 {
				return Err(DebugValidatorError::InstructionExecError(i, return_code.into(), the_big_log));
			}
		}
		let account_data_result = match return_choice {
			DebugLedgerAccountReturnChoice::All => {
				account_datas_changed
			},
			DebugLedgerAccountReturnChoice::Edited => {
				let mut result = HashMap::new();
				for (pubkey, old_data) in account_datas.into_iter() {
					let new_data = account_datas_changed.get(&pubkey).unwrap().clone();
					if new_data != old_data {
						result.insert(pubkey, new_data);
					}
				}
				result
			},
			DebugLedgerAccountReturnChoice::Only(pubkeys) => {
				let mut result = HashMap::new();
				for pubkey in pubkeys.into_iter() {
					result.insert(pubkey, account_datas_changed.get(&pubkey).unwrap().clone());
				}
				result
			}
		};
		Ok((account_data_result, the_big_log))
	}
}

#[derive(Debug, Default, BorshSerialize, BorshDeserialize)]
struct DebugLedgerState {
	#[borsh_skip]
	path: PathBuf,
	slot: u64
}
impl DebugLedgerState {
	pub async fn new(path: PathBuf) -> Result<Self, io::Error> {
		match fs::read(&path).await {
			Ok(data) => {
				let mut sayulf = Self::try_from_slice(&data)?;
				sayulf.path = path;
				Ok(sayulf)
			},
			Err(err) if err.kind() == io::ErrorKind::NotFound => {
				let mut sayulf = Self::default();
				sayulf.path = path;
				sayulf.slot = 1;
				Ok(sayulf)
			},
			Err(err) => Err(err),
		}
	}
	pub async fn save(&self) -> Result<(), io::Error> {
		fs::write(&self.path, self.try_to_vec()?).await
	}
	pub async fn inc_slot(&mut self) -> Result<(), io::Error> {
		self.slot += 1;
		self.save().await
	}
	pub fn slot(&self) -> u64 {
		self.slot
	}
}

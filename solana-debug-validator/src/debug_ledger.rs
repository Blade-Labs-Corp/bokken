use std::path::PathBuf;

use borsh::{BorshSerialize, BorshDeserialize};
use solana_debug_runtime::debug_env::DebugAccountData;
use solana_sdk::{pubkey, pubkey::Pubkey, system_program, program_error::ProgramError};
use tokio::fs;

use crate::error::DebugValidatorError;

pub struct DebugLedgerInitConfig {
	pub initial_mint: Pubkey,
	pub initial_mint_lamports: u64
}

pub struct DebugLedger {
	base_path: PathBuf,
	accounts_path: PathBuf
}
impl DebugLedger {
	pub async fn new(base_path: PathBuf, init_mint_config: Option<DebugLedgerInitConfig>) -> Result<Self, DebugValidatorError> {
		let accounts_path = {
			let mut p = base_path.clone();
			p.push("accounts");
			p
		};
		let sayulf = Self {
			base_path,
			accounts_path
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
		let account_path = {
			let mut p = self.accounts_path.clone();
			p.push(pubkey.to_string());
			p
		};
		let file_data = fs::read(account_path).await?;
		let file_data_parsed = DebugAccountData::try_from_slice(&file_data)?;
		Ok(file_data_parsed)
	}
}

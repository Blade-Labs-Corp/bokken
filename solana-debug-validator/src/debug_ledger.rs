use std::path::PathBuf;

use solana_debug_runtime::debug_env::DebugAccountData;
use solana_program::{pubkey::Pubkey, system_program};
use tokio::fs;

use crate::error::DebugValidatorError;

pub(crate) struct DebugLedgerInitConfig {
	initial_mint: Pubkey,
	initial_mint_lamports: u64
}

pub(crate) struct DebugLedger {
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
		match fs::create_dir(&base_path).await {
			Ok(_) => {
				// TODO: Verify integrity of save space?
			},
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				fs::create_dir(&accounts_path).await?;
				let init_mint_config = init_mint_config.ok_or(DebugValidatorError::InitConfigIsNone)?;

				let init_mint_account = DebugAccountData {
					lamports: init_mint_config.initial_mint_lamports,
					data: Vec::new(),
					owner: system_program::id(),
					executable: false,
					rent_epoch: 0
				};
			},
			Err(e) => {
				return Err(e.into())
			}
		}
		Ok(
			Self {
				base_path,
				accounts_path
			}
		)
	}
}

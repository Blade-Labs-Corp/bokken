use std::collections::HashMap;

use bokken_runtime::debug_env::{BorshAccountMeta, BokkenAccountData};
use solana_sdk::{pubkey::Pubkey, program_error::ProgramError};

pub fn assert_account_meta(
	metas: &Vec<BorshAccountMeta>,
	datas: &mut HashMap<Pubkey, BokkenAccountData>,
	index: usize,
	writable: bool,
	signer: bool
) -> Result<(Pubkey, BokkenAccountData), ProgramError> {
	let meta = metas.get(index).ok_or(ProgramError::NotEnoughAccountKeys)?;
	if writable && !meta.is_writable {
		// TODO: Better error code
		return Err(ProgramError::Custom(0));
	}
	if signer && !meta.is_signer {
		return Err(ProgramError::MissingRequiredSignature);
	}
	Ok((meta.pubkey, datas.remove(&meta.pubkey).ok_or(ProgramError::NotEnoughAccountKeys)?))
}

pub mod system_program;
pub trait NativeProgramStub: Send + Sync + std::fmt::Debug {
	fn clear_logs(&mut self);
	fn logs(&self) -> &Vec<String>;
	fn logs_mut(&mut self) -> &mut Vec<String>;
	fn msg(&mut self, msg: String) {
		self.logs_mut().push(format!("Program logged: {}", msg))
	}
	fn msg_str(&mut self, msg: &str) {
		self.logs_mut().push(format!("Program logged: {}", msg))
	}
	fn exec(
		&mut self,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: &mut HashMap<Pubkey, BokkenAccountData>
	) -> Result<(), ProgramError>;
}

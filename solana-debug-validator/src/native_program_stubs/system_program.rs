use std::collections::HashMap;

use solana_debug_runtime::debug_env::{BorshAccountMeta, DebugAccountData};
use solana_sdk::{program_error::ProgramError, system_instruction::SystemInstruction, pubkey::Pubkey};

use super::{NativeProgramStub, assert_account_meta};

const MAX_ACCOUNT_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct DebugSystemProgram {
	logs: Vec<String>
}
impl DebugSystemProgram {
	pub fn new() -> Self {
		Self {
			logs: Vec::new()
		}
	}
}
impl NativeProgramStub for DebugSystemProgram {
	fn clear_logs(&mut self) {
		self.logs.clear()
	}

	fn logs(&self) -> &Vec<String> {
		&self.logs
	}

	fn logs_mut(&mut self) -> &mut Vec<String> {
		&mut self.logs
	}

	fn exec(
		&mut self,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: &mut HashMap<Pubkey, DebugAccountData>
	) -> Result<(), ProgramError> {
		match bincode::deserialize::<SystemInstruction>(&instruction).map_err(|_|{ProgramError::InvalidInstructionData})? {
			SystemInstruction::CreateAccount {lamports, space, owner } => {
				let (
					funding_account_key,
					mut funding_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, true)?;
				let (
					new_account_key,
					mut new_account
				) = assert_account_meta(&account_metas, account_datas, 1, true, true)?;

				if new_account.data.len() > 0 {
					return Err(ProgramError::AccountAlreadyInitialized);
				}
				if space > MAX_ACCOUNT_SIZE {
					self.msg(format!("{} > {}", space, MAX_ACCOUNT_SIZE));
					return Err(ProgramError::InvalidRealloc);
				}
				funding_account.move_lamports(&mut new_account, lamports)?;
				new_account.owner = owner;
				new_account.data = vec![0; space as usize];
				
				account_datas.insert(funding_account_key, funding_account);
				account_datas.insert(new_account_key, new_account);
			},
			SystemInstruction::Assign { owner } => {
				let (
					account_key,
					mut account
				) = assert_account_meta(&account_metas, account_datas, 0, true, true)?;
				account.owner = owner;
				account_datas.insert(account_key, account);
			},
			SystemInstruction::Transfer { lamports } => {
				let (
					from_account_key,
					mut from_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, true)?;
				let (
					to_account_key,
					mut to_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, false)?;
				from_account.move_lamports(&mut to_account, lamports)?;

				account_datas.insert(from_account_key, from_account);
				account_datas.insert(to_account_key, to_account);
			},
			SystemInstruction::CreateAccountWithSeed { base, seed, lamports, space, owner } => {
				let (
					funding_account_key,
					mut funding_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, true)?;
				let (
					new_account_key,
					mut new_account
				) = assert_account_meta(&account_metas, account_datas, 1, true, false)?;
				if base != funding_account_key && !account_metas.get(2).ok_or(ProgramError::NotEnoughAccountKeys)?.is_signer {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if new_account_key != Pubkey::create_with_seed(&base, &seed, &owner)? {
					self.msg_str("Provided new account and derived seed don't match");
					return Err(ProgramError::InvalidSeeds);
				}

				if new_account.data.len() > 0 {
					return Err(ProgramError::AccountAlreadyInitialized);
				}
				if space > MAX_ACCOUNT_SIZE {
					self.msg(format!("{} > {}", space, MAX_ACCOUNT_SIZE));
					return Err(ProgramError::InvalidRealloc);
				}
				funding_account.move_lamports(&mut new_account, lamports)?;
				new_account.owner = owner;
				new_account.data = vec![0; space as usize];
				
				account_datas.insert(funding_account_key, funding_account);
				account_datas.insert(new_account_key, new_account);
			},
			SystemInstruction::Allocate { space } => {
				let (
					new_account_key,
					mut new_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, true)?;

				if new_account.data.len() > 0 {
					return Err(ProgramError::AccountAlreadyInitialized);
				}
				if space > MAX_ACCOUNT_SIZE {
					self.msg(format!("{} > {}", space, MAX_ACCOUNT_SIZE));
					return Err(ProgramError::InvalidRealloc);
				}
				new_account.data = vec![0; space as usize];
				
				account_datas.insert(new_account_key, new_account);
			},
			SystemInstruction::AllocateWithSeed { base, seed, space, owner } => {
				let (
					new_account_key,
					mut new_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, false)?;

				if new_account.data.len() > 0 {
					return Err(ProgramError::AccountAlreadyInitialized);
				}
				if space > MAX_ACCOUNT_SIZE {
					self.msg(format!("{} > {}", space, MAX_ACCOUNT_SIZE));
					return Err(ProgramError::InvalidRealloc);
				}

				if !account_metas.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?.is_signer {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if account_metas.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?.pubkey != base {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if new_account_key != Pubkey::create_with_seed(&base, &seed, &owner)? {
					self.msg_str("Provided new account and derived seed don't match");
					return Err(ProgramError::InvalidSeeds);
				}

				new_account.data = vec![0; space as usize];
				
				account_datas.insert(new_account_key, new_account);
			},
			SystemInstruction::AssignWithSeed { base, seed, owner } => {
				let (
					account_key,
					mut account
				) = assert_account_meta(&account_metas, account_datas, 0, true, false)?;

				if !account_metas.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?.is_signer {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if account_metas.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?.pubkey != base {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if account_key != Pubkey::create_with_seed(&base, &seed, &owner)? {
					self.msg_str("Provided new account and derived seed don't match");
					return Err(ProgramError::InvalidSeeds);
				}

				account.owner = owner;
				account_datas.insert(account_key, account);
			},
			SystemInstruction::TransferWithSeed { lamports, from_seed, from_owner } => {
				let (
					from_account_key,
					mut from_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, false)?;

				let base_meta = account_metas.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?;
				if !base_meta.is_signer {
					return Err(ProgramError::MissingRequiredSignature);
				}
				if from_account_key != Pubkey::create_with_seed(&base_meta.pubkey, &from_seed, &from_owner)? {
					self.msg_str("Provided new account and derived seed don't match");
					return Err(ProgramError::InvalidSeeds);
				}

				let (
					to_account_key,
					mut to_account
				) = assert_account_meta(&account_metas, account_datas, 0, true, false)?;
				from_account.move_lamports(&mut to_account, lamports)?;

				account_datas.insert(from_account_key, from_account);
				account_datas.insert(to_account_key, to_account);
			},
			_ => {
				self.msg_str("Unknown/Unimplemented SystemInstruction");
				return Err(ProgramError::InvalidInstructionData);
			}
		}
		Ok(())
	}
}

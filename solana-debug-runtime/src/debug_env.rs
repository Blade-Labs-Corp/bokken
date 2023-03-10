use std::collections::HashMap;

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{pubkey::Pubkey, instruction::AccountMeta, program_error::ProgramError};

/// The structure used to store a Solana account's information
#[derive(PartialEq, Eq, Debug, Clone, BorshSerialize, BorshDeserialize, Default)]
pub struct BokkenAccountData {
	// pub pubkey: Pubkey,
	pub lamports: u64,
	pub data: Vec<u8>,
	pub owner: Pubkey,
	pub executable: bool,
	pub rent_epoch: u64
}
impl BokkenAccountData {
	pub fn move_lamports<'a>(&'a mut self, to: &'a mut Self, amount: u64) -> Result<(), ProgramError> {
		if self.lamports < amount {
			return Err(ProgramError::InsufficientFunds)
		}
		self.lamports -= amount;
		to.lamports += amount;
		Ok(())
	}
}

/// Same as Solana's own `AccountMeta`, except this implements `BorshSerialize` and `BorshDeserialize`
#[derive(Debug, Default, PartialEq, Clone, BorshSerialize, BorshDeserialize)]
pub struct BorshAccountMeta {
    /// An account's public key.
    pub pubkey: Pubkey,
    /// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
    pub is_signer: bool,
    /// True if the account data or metadata may be mutated during program execution.
    pub is_writable: bool,
}
impl From<&AccountMeta> for BorshAccountMeta {
	fn from(meta: &AccountMeta) -> Self {
		Self {
			pubkey: meta.pubkey,
			is_signer: meta.is_signer,
			is_writable: meta.is_writable
		}
	}
}
impl From<AccountMeta> for BorshAccountMeta {
	fn from(meta: AccountMeta) -> Self {
		Self {
			pubkey: meta.pubkey,
			is_signer: meta.is_signer,
			is_writable: meta.is_writable
		}
	}
}
impl From<BorshAccountMeta> for AccountMeta {
	fn from(meta: BorshAccountMeta) -> Self {
		Self {
			pubkey: meta.pubkey,
			is_signer: meta.is_signer,
			is_writable: meta.is_writable
		}
	}
}


/// IPC message sent from a debuggable program to the main Bokken process.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum BokkenRuntimeMessage {
	Log {
		nonce: u64,
		message: String
	},
	Executed {
		nonce: u64,
		return_code: u64,
		account_datas: HashMap<Pubkey, BokkenAccountData>
	},
	CrossProgramInvoke {
		nonce: u64,
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, BokkenAccountData>,
		call_depth: u8
	}
}

/// IPC message send from the main Bokken process to a debuggable program
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum BokkenValidatorMessage {
	Invoke {
		nonce: u64,
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, BokkenAccountData>,
		call_depth: u8
	},
	CrossProgramInvokeResult {
		nonce: u64,
		return_code: u64,
		account_datas: HashMap<Pubkey, BokkenAccountData>
	}
}

use std::collections::HashMap;

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{pubkey::Pubkey, instruction::AccountMeta};

#[derive(PartialEq, Eq, Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct DebugAccountData {
	// pub pubkey: Pubkey,
	pub lamports: u64,
	pub data: Vec<u8>,
	pub owner: Pubkey,
	pub executable: bool,
	pub rent_epoch: u64
}

#[derive(Debug, Default, PartialEq, Clone, BorshSerialize, BorshDeserialize)]
pub struct BorshAccountMeta {
    /// An account's public key.
    pub pubkey: Pubkey,
    /// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
    pub is_signer: bool,
    /// True if the account data or metadata may be mutated during program execution.
    pub is_writable: bool,
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

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum DebugRuntimeMessage {
	Log {
		nonce: u64,
		message: String
	},
	Executed {
		nonce: u64,
		return_code: u64,
		account_datas: HashMap<Pubkey, DebugAccountData>
	}
}


#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum DebugValidatorMessage {
	Invoke {
		nonce: u64,
		program_id: Pubkey,
		instruction: Vec<u8>,
		account_metas: Vec<BorshAccountMeta>,
		account_datas: HashMap<Pubkey, DebugAccountData>,
		call_depth: u8
	}
}

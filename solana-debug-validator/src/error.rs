use solana_sdk::{transaction::TransactionError, sanitize::SanitizeError, program_error::ProgramError, pubkey::ParsePubkeyError};
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum BokkenError {
	#[error("This shouldn't happen!")]
	ShouldNotHappen,
	#[error("IO Error: {0}")]
	IO(#[from] io::Error),
	#[error("All initialization options must be specified if a pre-existing state doesn't exist at the path provided")]
	InitConfigIsNone,
	#[error("this feature is unimplemented")]
	Unimplemented,
	#[error("Base64 Error: {0}")]
	Base64DecodeError(#[from] base64::DecodeError),
	#[error("Base58 Error: {0}")]
	Base58DecodeError(#[from] bs58::decode::Error),
	#[error("Bincode Error: {0}")]
	BincodeDecodeError(#[from] bincode::Error),
	#[error("Solana tx error: {0}")]
	TransactionError(#[from] TransactionError),
	#[error("Sanitize error: {0}")]
	SanitizeError(#[from] SanitizeError),
	#[error("Pubkey parse error: {0}")]
	PubkeyParseError(#[from] ParsePubkeyError),
	#[error("Connection to program dropped while waiting for execution result")]
	ProgramClosedConnection,
	#[error("The program is stopping")]
	Stopping,
	#[error("Instruction #{0}: Program returned: {1}")]
	InstructionExecError(usize, ProgramError, Vec<String>)
	
}
impl From<BokkenError> for jsonrpsee::core::Error {
	fn from(err: BokkenError) -> Self {
		Self::Custom(err.to_string())
	}
}

use solana_sdk::{transaction::TransactionError, sanitize::SanitizeError, program_error::ProgramError};
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum DebugValidatorError {
	#[error("This shouldn't happen!")]
	ShouldNotHappen,
	#[error("IO Error: {0}")]
	IO(#[from] io::Error),
	#[error("Initialization config must be given when initializing")]
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
	#[error("Connection to program dropped while waiting for execution result")]
	ProgramClosedConnection,
	#[error("The program is stopping")]
	Stopping,
	#[error("Instruction #{0}: Program returned: {1}")]
	InstructionExecError(usize, ProgramError, Vec<String>)
	
}
impl From<DebugValidatorError> for jsonrpsee::core::Error {
	fn from(err: DebugValidatorError) -> Self {
		Self::Custom(err.to_string())
	}
}

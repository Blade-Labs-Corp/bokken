use solana_sdk::{transaction::TransactionError, sanitize::SanitizeError, program_error::ProgramError, pubkey::ParsePubkeyError};
use thiserror::Error;
use std::{io, backtrace::Backtrace, fmt::Display};

#[derive(Error, Debug)]
pub enum BokkenError {
	// Original errors
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
	InstructionExecError(usize, ProgramError, Vec<String>),

	// Errors during ledger lookup
	#[error("Couldn't serialize {0}; encoded size was {2} > {1}")]
	MaximumSerializationLengthExceeded(String, usize, usize),
	#[error("Cannot append data to the end of the index as the identifiers would be unsorted")]
	CannotAppendToIndex,
	#[error("Unexpected end of file")]
	UnexpectedEOF,
	#[error("Indexed file out of bounds index={0}, length={1}")]
	IndexFileOutOfBounds(usize, usize),
	#[error("Invalid signature length")]
	InvalidSignatureLength
}
impl From<BokkenError> for jsonrpsee::core::Error {
	fn from(err: BokkenError) -> Self {
		Self::Custom(err.to_string())
	}
}


#[derive(Debug)]
pub struct BokkenDetailedError {
	// TODO:Maybe switch this thing back to using the Error derive macro when this is stable, as apparently the
	//		Error derive macro for errors which contain backtraces depend on this:
	//		https://github.com/rust-lang/rust/issues/99301
	source: Box<BokkenError>,
	backtrace: Backtrace,
}
impl Display for BokkenDetailedError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		return Display::fmt(&self.source, f);
	}
}
impl From<BokkenError> for BokkenDetailedError {
	fn from(source_error: BokkenError) -> Self {
		Self {
			source: Box::new(source_error),
			backtrace: Backtrace::force_capture(),
		}
	}
}
impl From<BokkenDetailedError> for BokkenError {
	fn from(value: BokkenDetailedError) -> Self {
		eprintln!("Warning! Collapsing a BokkenDetailedError back into a BokkenError!");
		eprintln!("BokkenError: {}", value.source);
		eprintln!("backtrace: {}", value.backtrace);
		*value.source // Looks like if we consume stuff, we can do anything
	}
}

impl std::error::Error for BokkenDetailedError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		// TODO: have this reference the source property somehow
		Some(&self.source)
	}
}

// I should put a derive macro for this
impl From<io::Error> for BokkenDetailedError {
	fn from(value: io::Error) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<base64::DecodeError> for BokkenDetailedError {
	fn from(value: base64::DecodeError) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<bs58::decode::Error> for BokkenDetailedError {
	fn from(value: bs58::decode::Error) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<bincode::Error> for BokkenDetailedError {
	fn from(value: bincode::Error) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<TransactionError> for BokkenDetailedError {
	fn from(value: TransactionError) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<SanitizeError> for BokkenDetailedError {
	fn from(value: SanitizeError) -> Self {
		Self::from(BokkenError::from(value))
	}
}
impl From<ParsePubkeyError> for BokkenDetailedError {
	fn from(value: ParsePubkeyError) -> Self {
		Self::from(BokkenError::from(value))
	}
}

use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum DebugValidatorError {
	#[error("This shouldn't happen!")]
	ShouldNotHappen,
	#[error("IO Error: {0}")]
	IO(#[from] io::Error),
	#[error("Initialization config must be given when initializing")]
	InitConfigIsNone
}

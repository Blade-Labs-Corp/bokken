use std::path::PathBuf;

use color_eyre::eyre;
use solana_program::pubkey::Pubkey;
use bpaf::Bpaf;


pub mod executor;
pub mod debug_env;

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// A barebones emulated solana enviroment for quick e2e testing
struct CommandOptions {
	/// The unix socket created by the debug validator
	#[bpaf(short, long, argument::<PathBuf>("PATH"), fallback(PathBuf::from("solana-debug-validator.sock")))]
	socket_path: PathBuf,

   	/// Program ID of this program
	#[bpaf(short, long, argument::<Pubkey>("PUBKEY"))]
	program_id: Pubkey,
}

pub async fn debug_runtime_main() -> eyre::Result<()> {

	todo!()
}

#[macro_export]
macro_rules! debug_validator_program {
    ($program_crate_name:ident) => {
		extern crate $program_crate_name;

		#[tokio::main]
		async fn main() -> color_eyre::eyre::Result<()> {
			color_eyre::install()?;
			solana_debug_runtime::debug_runtime_main().await
		}
    };
}

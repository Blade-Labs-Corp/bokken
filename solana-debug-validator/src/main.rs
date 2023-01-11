use std::path::PathBuf;
use solana_program::pubkey::Pubkey;
use color_eyre::eyre::Result;
use tokio::net::UnixListener;
use tokio::fs;
use bpaf::Bpaf;

mod comm;
mod debug_ledger;
mod error;

use crate::debug_ledger::DebugLedger;

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// A barebones emulated solana enviroment for quick e2e testing
struct CommandOptions {
	/// Where the unix socket will be. Used to connect to debuggable programs.
	#[bpaf(short, long, argument::<PathBuf>("PATH"), fallback(PathBuf::from("solana-debug-validator.sock")))]
	socket_path: PathBuf,

   	/// Where the unix socket will be. Used to connect to debuggable programs.
	#[bpaf(short, long, argument::<PathBuf>("PATH"), fallback(PathBuf::from("not-ledeger")))]
	save_path: PathBuf,
}

async fn main_but_with_autocomplete() -> Result<()> {
	let opts = command_options().run();
	let ledger_env = DebugLedger::new(opts.save_path, None).await?;
	
	
	// let rpc_listner 
	let ipc_listener = UnixListener::bind(opts.socket_path)?;
	loop {
		match ipc_listener.accept().await {
			Ok((stream, _addr)) => {
				println!("new client!");
				
			}
			Err(e) => { /* connection failed */ }
		}
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	println!("Hello, world!");
	color_eyre::install()?;
	main_but_with_autocomplete().await
}

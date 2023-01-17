use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr, SocketAddrV4, SocketAddrV6};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use debug_ledger::DebugLedgerInitConfig;
use program_caller::ProgramCaller;
use solana_debug_runtime::ipc_comm::IPCComm;
use solana_sdk::{pubkey, pubkey::Pubkey};
use color_eyre::eyre::Result;
use tokio::net::UnixListener;
use tokio::fs;
use bpaf::Bpaf;
use tokio::task::JoinHandle;

mod error;
mod debug_ledger;
mod rpc_endpoint;
mod program_caller;

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

	#[bpaf(short, long, argument::<IpAddr>("IP ADDRESS"), fallback(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))))]
	listen_addr: IpAddr,

	#[bpaf(short, long, argument::<u16>("PORT"), fallback(8899))]
	listen_port: u16
}

async fn main_but_with_autocomplete() -> Result<()> {
	let opts = command_options().run();

	let ipc_listener = UnixListener::bind(opts.socket_path)?;
	let ledger = DebugLedger::new(
		opts.save_path,
		Some(DebugLedgerInitConfig {
			initial_mint: pubkey!("2iXtA8oeZqUU5pofxK971TCEvFGfems2AcDRaZHKD2pQ"),
			initial_mint_lamports: 10000000000000000
		})
	).await?;
	let program_caller = ProgramCaller::new(ipc_listener);
	
	rpc_endpoint::start_endpoint(
		match opts.listen_addr {
			IpAddr::V4(addr) => {
				SocketAddr::V4(SocketAddrV4::new(addr, opts.listen_port))
			},
			IpAddr::V6(addr) => {
				SocketAddr::V6(SocketAddrV6::new(addr, opts.listen_port, 0, 0))
			},
		},
		ledger,
		program_caller,
	).await?;
	Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
	println!("Hello, world!");
	color_eyre::install()?;
	main_but_with_autocomplete().await
}

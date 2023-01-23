
use std::net::{SocketAddr, IpAddr, Ipv4Addr, SocketAddrV4, SocketAddrV6};
use std::path::PathBuf;


use debug_ledger::BokkenLedgerInitConfig;
use program_caller::ProgramCaller;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::{pubkey};
use color_eyre::eyre::Result;
use tokio::net::UnixListener;

use bpaf::Bpaf;


mod error;
mod debug_ledger;
mod rpc_endpoint_structs;
mod rpc_endpoint;
mod native_program_stubs;
mod program_caller;

use crate::debug_ledger::BokkenLedger;


#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// A barebones emulated solana enviroment for quick e2e testing
struct CommandOptions {
	/// Where the unix socket will be. Used to connect to debuggable programs.
	/// (Default: solana-debug-validator.sock)
	#[bpaf(short, long, argument::<PathBuf>("PATH"), fallback(PathBuf::from("bokken.sock")))]
	socket_path: PathBuf,

   	/// Where to save the state of the Bokken ledger
	/// (Default: not-ledger)
	#[bpaf(short('S'), long, argument::<PathBuf>("PATH"), fallback(PathBuf::from("not-ledger")))]
	save_path: PathBuf,

	/// JSON-RPC IP address to listen to
	/// (Default: 127.0.0.1)
	#[bpaf(short('a'), long, argument::<IpAddr>("IP ADDRESS"), fallback(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))))]
	listen_addr: IpAddr,

	/// JSON-RPC IP port to listen to
	/// (Default: 8899)
	#[bpaf(short('p'), long, argument::<u16>("PORT"), fallback(8899))]
	listen_port: u16,

	/// If save-path doesn't already exist, initialize the following account with `initial-mint-lamports`
	#[bpaf(short('m'), long, argument::<Pubkey>("PUBKEY"))]
	initial_mint_pubkey: Option<Pubkey>,

	/// Amount to initialize `initial-mint-pubkey` with if save-path doesn't already exist
	/// (Default: 500000000000000000)
	#[bpaf(short('M'), long, argument::<u64>("LAMPORTS"), fallback(500000000000000000))]
	initial_mint_lamports: u64
}

#[tokio::main]
async fn main() -> Result<()> {
	println!("Is your program Bokken today?");
	color_eyre::install()?;

	let opts = command_options().run();
	let ipc_listener = UnixListener::bind(opts.socket_path)?;
	let ledger = BokkenLedger::new(
		opts.save_path,
		ProgramCaller::new(ipc_listener),
		opts.initial_mint_pubkey.map(|pubkey| {
			BokkenLedgerInitConfig {
				initial_mint: pubkey,
				initial_mint_lamports: opts.initial_mint_lamports
			}
		})
	).await?;
	
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
	).await?;
	Ok(())
}

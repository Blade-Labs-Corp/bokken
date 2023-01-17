use std::{path::PathBuf, sync::Arc};

use color_eyre::eyre;
use debug_env::{DebugValidatorMessage, DebugRuntimeMessage};
use executor::SolanaDebugContext;
use ipc_comm::IPCComm;
use solana_program::pubkey::Pubkey;
use bpaf::Bpaf;
use tokio::net::UnixStream;


pub mod executor;
pub mod debug_env;
pub mod ipc_comm;


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
	let opts = command_options().run();
	let mut comm = IPCComm::new(UnixStream::connect(opts.socket_path).await?);
	
	comm.send_msg(opts.program_id).await?;
	println!("DEBUG: debug_runtime_main: sent program id");
	while let Some(msg) = comm.until_recv_msg::<DebugValidatorMessage>().await? {
		match msg {
			DebugValidatorMessage::Invoke {
				nonce,
				program_id,
				instruction,
				account_metas,
				account_datas
			} => {
				println!("DEBUG: Got invoke request");
				let mut context = SolanaDebugContext::new(
					program_id,
					instruction,
					account_metas.into_iter().map(|v|{v.into()}).collect(),
					account_datas
				);
				let return_code = context.execute_sol_program().await;
				println!("DEBUG: program invoked! return code {}", return_code);
				comm.send_msg(DebugRuntimeMessage::Executed {
					nonce,
					return_code,
					account_datas: context.get_account_datas()
				}).await?;
			},
		}
	}
	Ok(())
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

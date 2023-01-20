use std::{path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}}, collections::{HashSet, HashMap}, thread::JoinHandle};

use color_eyre::eyre;
use debug_env::{DebugValidatorMessage, DebugRuntimeMessage, DebugAccountData};
use executor::SolanaDebugContext;
use ipc_comm::IPCComm;
use sol_syscalls::{DebugValidatorSyscalls, DebugValidatorSyscallMsg};
use solana_program::{pubkey::Pubkey, program_stubs::set_syscall_stubs};
use bpaf::Bpaf;
use tokio::{net::UnixStream, sync::{Mutex, mpsc}, task, join};


pub mod sol_syscalls;
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

async fn ipc_read_loop(
	comm: Arc<Mutex<IPCComm>>,
	syscall_sender: mpsc::Sender<DebugValidatorSyscallMsg>,
	invoke_result_senders: Arc<Mutex<HashMap<u64, mpsc::Sender<(u64, HashMap<Pubkey, DebugAccountData>)>>>>
) -> eyre::Result<()> {
	while let Some(msg) = comm.lock().await.until_recv_msg::<DebugValidatorMessage>().await? {
		match msg {
			DebugValidatorMessage::Invoke {
				nonce,
				program_id,
				instruction,
				account_metas,
				account_datas,
				call_depth
			} => {
				let context = SolanaDebugContext::new(
					program_id,
					instruction,
					account_metas.into_iter().map(|v|{v.into()}).collect(),
					account_datas,
					nonce,
					call_depth
				);
				syscall_sender.send(
					DebugValidatorSyscallMsg::PushContext{
						ctx: context,
						msg_sender_clone: syscall_sender.clone()
					}
				).await?;
			},
   			DebugValidatorMessage::CrossProgramInvokeResult {
				nonce,
				return_code,
				account_datas
			} => {
				if let Some(sender) = invoke_result_senders.lock().await.remove(&nonce) {
					sender.send((return_code, account_datas)).await?;
				}
			},
		}
	}
	Ok(())
}

pub async fn debug_runtime_main() -> eyre::Result<()> {
	let opts = command_options().run();
	let comm = Arc::new(Mutex::new(IPCComm::new(UnixStream::connect(opts.socket_path).await?)));
	{
		comm.lock().await.send_msg(opts.program_id).await?;
	}
	let (syscall_sender, syscall_receiver) = mpsc::channel::<DebugValidatorSyscallMsg>(1);
	let invoke_result_senders = Arc::new(Mutex::new(HashMap::new()));
	let syscall_mgr = Box::new(DebugValidatorSyscalls::new(
		comm.clone(),
		opts.program_id,
		invoke_result_senders.clone(),
		syscall_receiver
	));
	set_syscall_stubs(syscall_mgr);
	println!("DEBUG: debug_runtime_main: sent program id");

	// TODO: Listen for signals and exit gracefully
	ipc_read_loop(comm, syscall_sender, invoke_result_senders).await?;
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

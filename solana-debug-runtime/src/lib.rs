use std::{path::PathBuf, sync::{Arc}, collections::{HashMap}, time::Duration};

use color_eyre::eyre;
use debug_env::{BokkenValidatorMessage, BokkenAccountData};
use executor::BokkenSolanaContext;
use ipc_comm::IPCComm;
use sol_syscalls::{BokkenSyscalls, BokkenSyscallMsg};
use solana_program::{pubkey::Pubkey, program_stubs::set_syscall_stubs};
use bpaf::Bpaf;
use tokio::{net::UnixStream, sync::{Mutex, mpsc}, time::sleep};


pub mod sol_syscalls;
pub mod executor;
pub mod debug_env;
pub mod ipc_comm;


#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// A native-compiled Solana program to be used with Bokken
struct CommandOptions {
	/// The unix socket of the Bokken instance to link to
	#[bpaf(short, long, argument::<PathBuf>("PATH"))]
	socket_path: PathBuf,

   	/// Program ID of this program
	#[bpaf(short, long, argument::<Pubkey>("PUBKEY"))]
	program_id: Pubkey,
}

async fn ipc_read_loop(
	comm: Arc<Mutex<IPCComm>>,
	syscall_sender: mpsc::Sender<BokkenSyscallMsg>,
	invoke_result_senders: Arc<Mutex<HashMap<u64, mpsc::Sender<(u64, HashMap<Pubkey, BokkenAccountData>)>>>>
) -> eyre::Result<()> {
	loop {
		// We must poll or else we prevent Logs and CPIs from getting sent
		let msg = {
			let mut comm = comm.lock().await;
			if comm.stopped() {
				break;
			}
			if let Some(msg) = comm.recv_msg().await? {
				msg
			}else{
				sleep(Duration::from_millis(1)).await;
				continue;
			}
		};
		match msg {
			BokkenValidatorMessage::Invoke {
				nonce,
				program_id,
				instruction,
				account_metas,
				account_datas,
				call_depth
			} => {
				let context = BokkenSolanaContext::new(
					program_id,
					instruction,
					account_metas.into_iter().map(|v|{v.into()}).collect(),
					account_datas,
					nonce,
					call_depth
				);
				syscall_sender.send(
					BokkenSyscallMsg::PushContext{
						ctx: context,
						msg_sender_clone: syscall_sender.clone()
					}
				).await?;
			},
   			BokkenValidatorMessage::CrossProgramInvokeResult {
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

pub async fn bokken_runtime_main() -> eyre::Result<()> {
	let opts = command_options().run();
	let comm = Arc::new(Mutex::new(IPCComm::new(UnixStream::connect(opts.socket_path).await?)));
	{
		comm.lock().await.send_msg(opts.program_id).await?;
	}
	let (syscall_sender, syscall_receiver) = mpsc::channel::<BokkenSyscallMsg>(1);
	let invoke_result_senders = Arc::new(Mutex::new(HashMap::new()));
	let syscall_mgr = Box::new(BokkenSyscalls::new(
		comm.clone(),
		opts.program_id,
		invoke_result_senders.clone(),
		syscall_receiver
	));
	set_syscall_stubs(syscall_mgr);
	println!("DEBUG: bokken_runtime_main: sent program id");

	// TODO: Listen for signals and exit gracefully
	ipc_read_loop(comm, syscall_sender, invoke_result_senders).await?;
	Ok(())
}

#[macro_export]
macro_rules! bokken_program {
    ($program_crate_name:ident) => {
		extern crate $program_crate_name;

		#[tokio::main]
		async fn main() -> color_eyre::eyre::Result<()> {
			color_eyre::install()?;
			bokken_runtime::bokken_runtime_main().await
		}
    };
}

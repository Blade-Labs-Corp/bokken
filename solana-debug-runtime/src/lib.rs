use std::{path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}}, collections::HashSet, thread::JoinHandle};

use color_eyre::eyre;
use debug_env::{DebugValidatorMessage, DebugRuntimeMessage};
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
	syscall_sender: mpsc::Sender<DebugValidatorSyscallMsg>
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
				println!("DEBUG: Got invoke request");
				todo!();
				/* 
				syscall_sender.send(
					DebugValidatorSyscallMsg::PushContext{ ctx: DebugValidatorSyscallContext {
						nonce,
						stack_height: call_depth,
						valid_writables: {
							let mut pubkeys = HashSet::new();
							for meta in account_metas.iter() {
								if meta.is_signer {
									pubkeys.insert(meta.pubkey.clone());
								}
							}
							pubkeys
						},
						valid_signers: {
							let mut pubkeys = HashSet::new();
							for meta in account_metas.iter() {
								if meta.is_signer {
									pubkeys.insert(meta.pubkey.clone());
								}
							}
							pubkeys
						},
					}}
				).await?;
				let mut context = SolanaDebugContext::new(
					program_id,
					instruction,
					account_metas.into_iter().map(|v|{v.into()}).collect(),
					account_datas,
					call_depth
				);
				// TODO: Do not await this
				let return_code = context.execute_sol_program().await;
				syscall_sender.send(
					DebugValidatorSyscallMsg::PopContext
				).await?;
				println!("DEBUG: program invoked! return code {}", return_code);
				{
					let mut comm = comm.lock().await;
					comm.send_msg(DebugRuntimeMessage::Executed {
						nonce,
						return_code,
						account_datas: context.get_account_datas()
					}).await?;
				}
				*/
			},
   			DebugValidatorMessage::CrossProgramInvokeResult {
				nonce,
				return_code,
				account_datas
			} => {
				todo!()
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
	let syscall_mgr = Box::new(DebugValidatorSyscalls::new(
		comm.clone(),
		opts.program_id,
		syscall_receiver
	));
	set_syscall_stubs(syscall_mgr);
	println!("DEBUG: debug_runtime_main: sent program id");

	// TODO: Listen for signals and exit gracefully
	

	
	loop {
		let msg = {
			let mut comm = comm.lock().await;
			let msg = comm.recv_msg::<DebugValidatorMessage>().await?;
			if msg.is_none() {
				continue;
			}
			msg.unwrap()
		};
		
	}
	// Ok(())
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

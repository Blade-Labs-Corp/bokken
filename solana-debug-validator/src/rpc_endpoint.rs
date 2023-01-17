use color_eyre::eyre;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::{proc_macros::rpc, core::async_trait, core::RpcResult};
use solana_debug_runtime::debug_env::BorshAccountMeta;
use solana_sdk::instruction::InstructionError;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sanitize::Sanitize;
use solana_sdk::transaction::{Transaction, TransactionError};
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;
use jsonrpsee::server::logger::{HttpRequest, MethodKind, TransportProtocol, Logger};
use jsonrpsee::types::Params;

use crate::debug_ledger::{DebugLedger, DebugLedgerInstruction, DebugLedgerAccountReturnChoice};
use crate::error::DebugValidatorError;
use crate::program_caller::ProgramCaller;


// Generate both server and client implementations, prepend all the methods with `foo_` prefix.
#[rpc(server)]
pub trait SolanaDebuggerRpc {
	#[method(name = "getLatestBlockhash")]
	async fn get_latest_blockhash(&self, config: Option<RpcGetLatestBlockhashRequest>) -> RpcResult<RpcGetLatestBlockhashResponse>;
	#[method(name = "getVersion")]
	fn get_version(&self) -> RpcResult<RpcVersionResponse>;
	#[method(name = "simulateTransaction")]
	async fn simulate_transaction(&self, tx_data: String, config: Option<RpcSimulateTransactionRequest>) -> RpcResult<RpcSimulateTransactionResponse>;
}

// start-common
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RpcBinaryEncoding {
	Base64,
	Base58,
	// #[serde(rename = "jsonParsed")]
	// JsonParsed
}
impl Default for RpcBinaryEncoding {
	fn default() -> Self {
		Self::Base64
	}
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RpcCommitment {
	Finalized,
	Confirmed,
	Processed
}
impl Default for RpcCommitment {
	fn default() -> Self {
		Self::Finalized
	}
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcResponseContext {
	slot: u64
}
// end-common
	
// start-getVersion
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RpcVersionResponse {
	solana_core: String,
	feature_set: u32
}
// end-getVersion

// start-getLatestBlockhash
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashRequest {
	commitment: RpcCommitment,
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashResponse {
	context: RpcResponseContext,
	value: RpcGetLatestBlockhashResponseValue
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashResponseValue {
	blockhash: String,
	last_valid_block_height: u64
}

// end-getLatestBlockHash

// start-simulateTransaction
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionRequest {
	#[serde(default)]
	sig_verify: bool,
	#[serde(default)]
	commitment: RpcCommitment,
	encoding: Option<RpcBinaryEncoding>,
	#[serde(default)]
	replace_recent_blockhash: bool,
	#[serde(default)]
	accounts: RpcSimulateTransactionRequestAccounts,
	#[serde(default)]
	min_context_slot: u64
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionRequestAccounts {
	encoding: RpcBinaryEncoding,
	addresses: Vec<Pubkey>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponse {
	context: RpcResponseContext,
	value: RpcSimulateTransactionResponseValue
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseValue {
	err: Option<solana_sdk::transaction::TransactionError>,
	logs: Option<Vec<String>>,
	accounts: Option<Vec<RpcSimulateTransactionResponseAccounts>>,
	units_consumed: Option<u64>,
	return_data: Option<RpcSimulateTransactionResponseReturnData>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseAccounts {
	lamports: u64,
	owner: String,
	data: (String, RpcBinaryEncoding),
	executable: bool,
	rent_epoch: u64
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseReturnData {
	program_id: String,
	data: (String, RpcBinaryEncoding)
}
// end-simulateTransaction

pub struct SolanaDebuggerRpcImpl {
	ledger: Arc<Mutex<DebugLedger>>
}
impl SolanaDebuggerRpcImpl {
	fn new(ledger: DebugLedger) -> Self {
		Self {
			ledger: Arc::new(Mutex::new(ledger))
		}
	}
	async fn _simulate_transaction(
		&self,
		tx_data: String,
		config: Option<RpcSimulateTransactionRequest>
	) -> Result<RpcSimulateTransactionResponse, DebugValidatorError> {
		let config = config.unwrap_or_default();
		
		// tx encoding has a default encoding type compared to everything else, woohoo!
		let tx: Transaction = bincode::deserialize(&match config.encoding.unwrap_or(RpcBinaryEncoding::Base58) {
			RpcBinaryEncoding::Base58 => {
				bs58::decode(tx_data).into_vec()?
			}
			RpcBinaryEncoding::Base64 => {
				base64::decode(tx_data)?
			}
		})?;

		// Verify the message isn't garbage
		tx.message.sanitize()?;
		if config.sig_verify {
			tx.verify()?;
		}
		if config.replace_recent_blockhash {
			println!("Warning: simulate_transaction: config.replace_recent_blockhash not considered!");
		}
		
		let account_pubkeys = &tx.message.account_keys;

		let mut ledger = self.ledger.lock().await;
		let ixs = tx.message.instructions.iter().map(|ix| {
			// Alright to directly index these since the message was sanitized earlier
			let program_id = account_pubkeys[ix.program_id_index as usize];
			// ChatGPT Assistant told me to do it this way
			let account_metas = ix.accounts.iter().map(|account_index|{
				// tx.message.header.
				BorshAccountMeta {
					pubkey: account_pubkeys[*account_index as usize],
					is_signer: tx.message.is_signer(*account_index as usize),
					is_writable: tx.message.is_writable(*account_index as usize)
				}

			}).collect::<Vec<BorshAccountMeta>>();
			DebugLedgerInstruction {
				program_id,
				account_metas,
				data: ix.data.clone()
			}
		}).collect();

		match ledger.execute_instructions(
			ixs,
			DebugLedgerAccountReturnChoice::Only(config.accounts.addresses.clone())
		).await {
			Ok((states, logs)) => {
				Ok(
					RpcSimulateTransactionResponse {
						context: RpcResponseContext { slot: ledger.slot() },
						value: RpcSimulateTransactionResponseValue {
							err: None,
							logs: Some(logs),
							accounts: Some(config.accounts.addresses.iter().map(|pubkey| {
								let state = states.get(pubkey).unwrap();
								RpcSimulateTransactionResponseAccounts{
									lamports: state.lamports,
									owner: state.owner.to_string(),
									data: (
										match config.accounts.encoding {
											RpcBinaryEncoding::Base64 => {
												base64::encode(&state.data)
											},
											RpcBinaryEncoding::Base58 => {
												bs58::encode(&state.data).into_string()
											},
										},
										config.accounts.encoding
									),
									executable: state.executable,
									rent_epoch: state.rent_epoch,
								}
							}).collect()),
							units_consumed: Some(0),
							return_data: None, // todo
						}
					}
				)
			},
			Err(e) => {
				match e {
					DebugValidatorError::InstructionExecError(index, program_error, logs) => {
						Ok(
							RpcSimulateTransactionResponse {
								context: RpcResponseContext { slot: ledger.slot() },
								value: RpcSimulateTransactionResponseValue {
									err: Some(TransactionError::InstructionError(index as u8, match program_error {
										// Why is there no "Into" definition for ProgramError -> InstructionError??
										ProgramError::Custom(n) => InstructionError::Custom(n),
										ProgramError::InvalidArgument => InstructionError::InvalidArgument,
										ProgramError::InvalidInstructionData => InstructionError::InvalidInstructionData,
										ProgramError::InvalidAccountData => InstructionError::InvalidAccountData,
										ProgramError::AccountDataTooSmall => InstructionError::AccountDataTooSmall,
										ProgramError::InsufficientFunds => InstructionError::InsufficientFunds,
										ProgramError::IncorrectProgramId => InstructionError::IncorrectProgramId,
										ProgramError::MissingRequiredSignature => InstructionError::MissingRequiredSignature,
										ProgramError::AccountAlreadyInitialized => InstructionError::AccountAlreadyInitialized,
										ProgramError::UninitializedAccount => InstructionError::UninitializedAccount,
										ProgramError::NotEnoughAccountKeys => InstructionError::NotEnoughAccountKeys,
										ProgramError::AccountBorrowFailed => InstructionError::AccountBorrowFailed,
										ProgramError::MaxSeedLengthExceeded => InstructionError::MaxSeedLengthExceeded,
										ProgramError::InvalidSeeds => InstructionError::InvalidSeeds,
										ProgramError::BorshIoError(s) => InstructionError::BorshIoError(s),
										ProgramError::AccountNotRentExempt => InstructionError::AccountNotRentExempt,
										ProgramError::UnsupportedSysvar => InstructionError::UnsupportedSysvar,
										ProgramError::IllegalOwner => InstructionError::IllegalOwner,
										ProgramError::MaxAccountsDataSizeExceeded => InstructionError::MaxAccountsDataSizeExceeded,
										ProgramError::InvalidRealloc => InstructionError::InvalidRealloc,
									})),
									logs: Some(logs),
									accounts: None,
									units_consumed: Some(0),
									return_data: None, // todo
								}
							}
						)
					},
					_ => {Err(e)}
				}
			},
		}
	}
}

// Note that the trait name we use is `MyRpcServer`, not `MyRpc`!
#[async_trait]
impl SolanaDebuggerRpcServer for SolanaDebuggerRpcImpl {
	async fn get_latest_blockhash(&self, _config: Option<RpcGetLatestBlockhashRequest>) -> RpcResult<RpcGetLatestBlockhashResponse> {
		let ledger = self.ledger.lock().await;
		Ok(
			RpcGetLatestBlockhashResponse {
				context: RpcResponseContext {
					slot: ledger.slot()
				},
				value: RpcGetLatestBlockhashResponseValue {
					blockhash: bs58::encode(ledger.blockhash()).into_string(),
					last_valid_block_height: 100
				}
			}
		)
	}
	fn get_version(&self) -> RpcResult<RpcVersionResponse> {
		Ok(
			RpcVersionResponse {
				solana_core: "1.13.5+debug-validator-0.0.1".to_string(),
				feature_set: 0
			}
		)
	}
	
	async fn simulate_transaction(
		&self,
		tx_data: String,
		config: Option<RpcSimulateTransactionRequest>
	) -> RpcResult<RpcSimulateTransactionResponse> {
		Ok(self._simulate_transaction(tx_data, config).await?)
	}
}


/// Example logger to keep a watch on the number of total threads started in the system.
#[derive(Clone)]
struct MyRpcLogger;
impl Logger for MyRpcLogger {
	type Instant = std::time::Instant;
	
	fn on_connect(&self, _remote_addr: SocketAddr, _headers: &HttpRequest, _t: TransportProtocol) {
		//println!("[MyRpcLogger::on_connect] remote_addr {:?}, headers: {:?}", remote_addr, headers);
	}

	fn on_call(&self, method: &str, params: Params, kind: MethodKind, _t: TransportProtocol) {
		println!("[JSON RPC Call]: method: {:?}, params: {:?}, kind: {:?}", method, params, kind);
	}
	fn on_request(&self, _t: TransportProtocol) -> Self::Instant {
		Self::Instant::now()
	}
	fn on_result(&self, _name: &str, _succees: bool, _started_at: Self::Instant, _t: TransportProtocol) {
		
	}
	fn on_response(&self, _result: &str, _started_at: Self::Instant, _t: TransportProtocol) {
		
	}
	fn on_disconnect(&self, _remote_addr: SocketAddr, _t: TransportProtocol) {
		
	}
}


// use crate::error::DebugValidatorError;
pub async fn start_endpoint(
	addr: SocketAddr,
	ledger: DebugLedger
) -> eyre::Result<()> {
	let server = ServerBuilder::default().set_logger(MyRpcLogger).build(addr).await?;
	//let addr = server.local_addr().unwrap();
	let server_handle = server.start(
		SolanaDebuggerRpcImpl::new(
			ledger
		).into_rpc()
	)?;
	server_handle.stopped().await;
	println!("Server stopped");
	Ok(())
}

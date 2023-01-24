use color_eyre::eyre;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::{proc_macros::rpc, core::async_trait, core::RpcResult};
use bokken_runtime::debug_env::BorshAccountMeta;
use solana_sdk::instruction::InstructionError;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sanitize::Sanitize;
use solana_sdk::transaction::{Transaction, TransactionError};
use tokio::sync::Mutex;

use std::net::SocketAddr;

use std::str::FromStr;
use std::sync::Arc;
use jsonrpsee::server::logger::{HttpRequest, MethodKind, TransportProtocol, Logger};
use jsonrpsee::types::Params;

use crate::debug_ledger::{BokkenLedger, BokkenLedgerInstruction, BokkenLedgerAccountReturnChoice};
use crate::error::BokkenError;

use crate::rpc_endpoint_structs::{RpcGetLatestBlockhashRequest, RpcVersionResponse, RpcGetLatestBlockhashResponse, RpcGetLatestBlockhashResponseValue, RpcResponseContext, RpcSimulateTransactionRequest, RpcSimulateTransactionResponse, RpcBinaryEncoding, RpcSimulateTransactionResponseValue, RpcSimulateTransactionResponseAccounts, RPCBinaryEncodedString, RpcGetAccountInfoRequest, RpcGetAccountInfoResponse, RpcGetBalanceResponse, RpcGetBalanceRequest, RpcGetAccountInfoResponseValue, RpcGenericConfigRequest, RpcSendTransactionRequest};


// Generate both server and client implementations, prepend all the methods with `foo_` prefix.
#[rpc(server)]
pub trait SolanaDebuggerRpc {
	#[method(name = "getAccountInfo")]
	async fn get_account_info(&self, pubkey: String, config: Option<RpcGetAccountInfoRequest>) -> RpcResult<RpcGetAccountInfoResponse>;
	#[method(name = "getBalance")]
	async fn get_balance(&self, pubkey: String, config: Option<RpcGetBalanceRequest>) -> RpcResult<RpcGetBalanceResponse>;
	#[method(name = "getBlockHeight")]
	async fn get_block_height(&self, _config: Option<RpcGetBalanceRequest>) -> RpcResult<u64>;
	#[method(name = "getLatestBlockhash")]
	async fn get_latest_blockhash(&self, config: Option<RpcGetLatestBlockhashRequest>) -> RpcResult<RpcGetLatestBlockhashResponse>;
	#[method(name = "getMinimumBalanceForRentExemption")]
	async fn get_min_balance_for_rent_exemption(&self, size: u64, config: Option<RpcGenericConfigRequest>) -> RpcResult<u64>;

	#[method(name = "getVersion")]
	fn get_version(&self) -> RpcResult<RpcVersionResponse>;
	#[method(name = "sendTransaction")]
	async fn send_transaction(&self, tx_data: String, config: Option<RpcSendTransactionRequest>) -> RpcResult<String>;
	#[method(name = "simulateTransaction")]
	async fn simulate_transaction(&self, tx_data: String, config: Option<RpcSimulateTransactionRequest>) -> RpcResult<RpcSimulateTransactionResponse>;
}

pub struct SolanaDebuggerRpcImpl {
	ledger: Arc<Mutex<BokkenLedger>>
}
impl SolanaDebuggerRpcImpl {
	fn new(ledger: BokkenLedger) -> Self {
		Self {
			ledger: Arc::new(Mutex::new(ledger))
		}
	}
	async fn _get_account_info(&self, pubkey: String, config: Option<RpcGetAccountInfoRequest>) -> Result<RpcGetAccountInfoResponse, BokkenError> {
		let pubkey = Pubkey::from_str(&pubkey)?;
		let config = config.unwrap_or_default();
		let ledger = self.ledger.lock().await;
		let data = ledger.read_account(&pubkey).await?;
		Ok(
			RpcGetAccountInfoResponse {
				context: RpcResponseContext { slot: ledger.slot() },
				value: if data.lamports == 0 {
					// BokkenLedger returns fake data if the account doesn't exist, so we'll just return none here
					None
				}else{
					Some(
						RpcGetAccountInfoResponseValue {
							lamports: data.lamports,
							owner: data.owner.to_string(),
							data: RPCBinaryEncodedString::from_bytes(&data.data, config.encoding),
							executable: data.executable,
							rent_epoch: data.rent_epoch,
						}
					)
				}
			}
		)
	}
	async fn _get_balance(&self, pubkey: String, config: Option<RpcGetBalanceRequest>) -> Result<RpcGetBalanceResponse, BokkenError> {
		let pubkey = Pubkey::from_str(&pubkey)?;
		let _config = config.unwrap_or_default();
		let ledger = self.ledger.lock().await;
		Ok(
			RpcGetBalanceResponse {
				context: RpcResponseContext { slot: ledger.slot() },
				value: ledger.read_account(&pubkey).await?.lamports
			}
		)
	}
	async fn _send_transaction(
		&self,
		tx_data: String,
		config: Option<RpcSendTransactionRequest>
	) -> Result<String, BokkenError> {
		let config = config.unwrap_or_default();
		// tx encoding has a default encoding type compared to everything else, woohoo!
		let tx: Transaction = bincode::deserialize(
			&config.encoding.unwrap_or(RpcBinaryEncoding::Base58).decode_bytes(&tx_data)?
		)?;

		// Verify the message isn't garbage. Note how "skip preflight" is ignored. Either we succeeded or we don't.
		tx.sanitize()?;
		tx.verify()?;

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
			BokkenLedgerInstruction {
				program_id,
				account_metas,
				data: ix.data.clone()
			}
		}).collect();

		let _ = ledger.execute_instructions(
			&tx.message.account_keys[0],
			ixs,
			BokkenLedgerAccountReturnChoice::None,
			true
		).await?;
		// The documented response is to just reply with the tx signature, so we just do that
		Ok(bs58::encode(tx.signatures[0]).into_string())
	}
	async fn _simulate_transaction(
		&self,
		tx_data: String,
		config: Option<RpcSimulateTransactionRequest>
	) -> Result<RpcSimulateTransactionResponse, BokkenError> {
		let config = config.unwrap_or_default();
		let config_account_addresses = {
			let mut config_account_addresses = Vec::new();
			for pubkey_string in config.accounts.addresses.iter() {
				config_account_addresses.push(Pubkey::from_str(pubkey_string)?);
			}
			config_account_addresses
		};
			
		
		// tx encoding has a default encoding type compared to everything else, woohoo!
		let tx: Transaction = bincode::deserialize(
			&config.encoding.unwrap_or(RpcBinaryEncoding::Base58).decode_bytes(&tx_data)?
		)?;

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
			BokkenLedgerInstruction {
				program_id,
				account_metas,
				data: ix.data.clone()
			}
		}).collect();

		match ledger.execute_instructions(
			&tx.message.account_keys[0],
			ixs,
			BokkenLedgerAccountReturnChoice::Only(config_account_addresses.clone()),
			false
		).await {
			Ok((states, logs)) => {
				Ok(
					RpcSimulateTransactionResponse {
						context: RpcResponseContext { slot: ledger.slot() },
						value: RpcSimulateTransactionResponseValue {
							err: None,
							logs: Some(logs),
							accounts: Some(config_account_addresses.iter().map(|pubkey| {
								let state = states.get(pubkey).unwrap();
								RpcSimulateTransactionResponseAccounts{
									lamports: state.lamports,
									owner: state.owner.to_string(),
									data: RPCBinaryEncodedString::from_bytes(&state.data, config.accounts.encoding),
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
					BokkenError::InstructionExecError(index, program_error, logs) => {
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
	async fn get_account_info(&self, pubkey: String, config: Option<RpcGetAccountInfoRequest>) -> RpcResult<RpcGetAccountInfoResponse> {
		Ok(self._get_account_info(pubkey, config).await?)
	}
	async fn get_balance(&self, pubkey: String, config: Option<RpcGetBalanceRequest>) -> RpcResult<RpcGetBalanceResponse> {
		Ok(self._get_balance(pubkey, config).await?)
	}
	async fn get_min_balance_for_rent_exemption(&self, size: u64, _config: Option<RpcGenericConfigRequest>) -> RpcResult<u64> {
		Ok(self.ledger.lock().await.calc_min_balance_for_rent_exemption(size))
	}
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
	async fn get_block_height(&self, _config: Option<RpcGetBalanceRequest>) -> RpcResult<u64> {
		Ok(self.ledger.lock().await.slot())
	}
	fn get_version(&self) -> RpcResult<RpcVersionResponse> {
		Ok(
			RpcVersionResponse {
				solana_core: "1.13.5+debug-validator-0.0.1".to_string(),
				feature_set: 0
			}
		)
	}
	async fn send_transaction(&self, tx_data: String, config: Option<RpcSendTransactionRequest>) -> RpcResult<String> {
		Ok(self._send_transaction(tx_data, config).await?)
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


// use crate::error::BokkenError;
pub async fn start_endpoint(
	addr: SocketAddr,
	ledger: BokkenLedger
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

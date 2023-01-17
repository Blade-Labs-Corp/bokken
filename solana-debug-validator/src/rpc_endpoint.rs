use color_eyre::eyre;
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::{proc_macros::rpc, core::async_trait, core::RpcResult};
use solana_debug_runtime::debug_env::BorshAccountMeta;
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

use crate::debug_ledger::DebugLedger;
use crate::error::DebugValidatorError;
use crate::program_caller::ProgramCaller;


// Generate both server and client implementations, prepend all the methods with `foo_` prefix.
#[rpc(server)]
pub trait SolanaDebuggerRpc {
	#[method(name = "getLatestBlockhash")]
	fn get_latest_blockhash(&self, config: Option<RpcGetLatestBlockhashRequest>) -> RpcResult<RpcGetLatestBlockhashResponse>;
	#[method(name = "getVersion")]
	fn get_version(&self) -> RpcResult<RpcVersionResponse>;
	#[method(name = "simulateTransaction")]
	async fn simulate_transaction(&self, tx_data: String, config: Option<RpcSimulateTransactionRequest>) -> RpcResult<RpcSimulateTransactionResponse>;
}

// start-common
#[derive(serde::Serialize, serde::Deserialize, Debug)]
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
	context: RpcGetLatestBlockhashResponseContext,
	value: RpcGetLatestBlockhashResponseValue
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashResponseContext {
	slot: u64
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
	err: Option<solana_sdk::transaction::TransactionError>,
	logs: Option<Vec<String>>,
	accounts: Option<RpcSimulateTransactionResponseAccounts>,
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
	program_caller: Arc<Mutex<ProgramCaller>>,
	ledger: Arc<Mutex<DebugLedger>>
}
impl SolanaDebuggerRpcImpl {
	fn new(ledger: DebugLedger, program_caller: ProgramCaller) -> Self {
		Self {
			program_caller: Arc::new(Mutex::new(program_caller)),
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
		if config.sig_verify {
			tx.verify()?;
		}
		if config.replace_recent_blockhash {
			println!("Warning: simulate_transaction: config.replace_recent_blockhash not considered!");
		}
		tx.message.sanitize()?;
		let account_pubkeys = &tx.message.account_keys;
		println!("DEBUG: {} ix(s) in tx", tx.message.instructions.len());
		for (i, ix) in tx.message.instructions.iter().enumerate() {
			
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
			
			// TODO: Do something with these
			println!("DEBUG: ix #{}", i);
			println!("\tprogram_id #{}", program_id);
			println!("\taccount_metas: {:?}", account_metas);
			println!("\tdata: {:x?}", ix.data);

			let ledger = self.ledger.lock().await;
			println!("DEBUG: locked ledger");
			let mut program_caller = self.program_caller.lock().await;
			println!("DEBUG: locked program caller");
			let mut account_datas = HashMap::new();
			for meta in account_metas.iter() {
				if !account_datas.contains_key(&meta.pubkey) {
					account_datas.insert(
						meta.pubkey,
						ledger.read_account(&meta.pubkey).await?
					);
				}
			}
			println!("DEBUG: read account states");
			let (return_code, account_datas) = program_caller.call_program(program_id, ix.data.clone(), account_metas, account_datas).await?;
			println!("DEBUG: called program! Return code {}", return_code);
			println!("DEBUG: New state {:x?}", account_datas);
		}
		// let parsed_tx_data = serde::Deserialize::deserialize::<solana_sdk::transaction::Transaction>(tx_data).unwrap();
		
		

		Err(DebugValidatorError::Unimplemented)
	}
}

// Note that the trait name we use is `MyRpcServer`, not `MyRpc`!
#[async_trait]
impl SolanaDebuggerRpcServer for SolanaDebuggerRpcImpl {
	fn get_latest_blockhash(&self, _config: Option<RpcGetLatestBlockhashRequest>) -> RpcResult<RpcGetLatestBlockhashResponse> {
		Ok(
			RpcGetLatestBlockhashResponse {
				context: RpcGetLatestBlockhashResponseContext {
					slot: 1
				},
				value: RpcGetLatestBlockhashResponseValue {
					blockhash: "11111111111111111111111111111111".to_string(),
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
	ledger: DebugLedger,
	program_caller: ProgramCaller
) -> eyre::Result<()> {
	let server = ServerBuilder::default().set_logger(MyRpcLogger).build(addr).await?;
	//let addr = server.local_addr().unwrap();
	let server_handle = server.start(
		SolanaDebuggerRpcImpl::new(
			ledger,
			program_caller
		).into_rpc()
	)?;
	server_handle.stopped().await;
	println!("Server stopped");
	Ok(())
}

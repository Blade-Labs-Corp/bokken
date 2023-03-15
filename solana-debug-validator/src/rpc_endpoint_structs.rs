use serde_with::{serde_as, DefaultOnNull};
use solana_sdk::transaction::TransactionError;

use crate::error::BokkenError;



// start-common
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RpcBinaryEncoding {
	Base64,
	Base58,
	#[serde(rename = "base64+zstd")]
	Base64Compressed
	// #[serde(rename = "jsonParsed")]
	// JsonParsed
}
impl RpcBinaryEncoding {
	pub fn decode_bytes(&self, data: &String) -> Result<Vec<u8>, BokkenError> {
		match self {
			RpcBinaryEncoding::Base58 => {
				Ok(bs58::decode(data).into_vec()?)
			}
			RpcBinaryEncoding::Base64 => {
				Ok(base64::decode(data)?)
			}
			RpcBinaryEncoding::Base64Compressed => {
				Ok(
					zstd::decode_all(
						base64::decode(data)?.as_slice()
					)?
				)
			},
		}
	}
}
impl Default for RpcBinaryEncoding {
	fn default() -> Self {
		Self::Base64
	}
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct RPCBinaryEncodedString (String, RpcBinaryEncoding);
impl RPCBinaryEncodedString {
	pub fn from_bytes(data: &[u8], encoding: RpcBinaryEncoding) -> Self {
		Self(
			match &encoding {
				RpcBinaryEncoding::Base64Compressed => {
					base64::encode(
						zstd::encode_all(
							data,
							0
						).expect("zstd to not fail")
					)
				},
				RpcBinaryEncoding::Base64 => {
					base64::encode(data)
				},
				RpcBinaryEncoding::Base58 => {
					bs58::encode(data).into_string()
				}
			},
			encoding
		)
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
	pub slot: u64
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcDataSlice {
	pub offset: usize,
	pub length: usize
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGenericConfigRequest {
	pub commitment: RpcCommitment
}
// end-common

// start-getAccountInfo
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetAccountInfoRequest {
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub commitment: RpcCommitment,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub encoding: RpcBinaryEncoding,
	pub data_slice: Option<RpcDataSlice>,
	#[serde(default)]
	#[serde_as(deserialize_as  = "DefaultOnNull")]
	pub min_context_slot: u64
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetAccountInfoResponse {
	pub context: RpcResponseContext,
	pub value: Option<RpcGetAccountInfoResponseValue>
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetAccountInfoResponseValue {
	pub lamports: u64,
	pub owner: String,
	pub data: RPCBinaryEncodedString,
	pub executable: bool,
	pub rent_epoch: u64
}

// end-getAccountInfo

// start-getBalance
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetBalanceRequest {
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub commitment: RpcCommitment,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub min_context_slot: u64
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetBalanceResponse {
	pub context: RpcResponseContext,
	pub value: u64
}
// end-getBalance


// start-getVersion
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RpcVersionResponse {
	pub solana_core: String,
	pub feature_set: u32
}
// end-getVersion

// start-getLatestBlockhash
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashRequest {
	pub commitment: RpcCommitment,
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashResponse {
	pub context: RpcResponseContext,
	pub value: RpcGetLatestBlockhashResponseValue
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetLatestBlockhashResponseValue {
	pub blockhash: String,
	pub last_valid_block_height: u64
}

// end-getLatestBlockHash

// start-sendTransaction
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSendTransactionRequest {
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub skip_verify: bool,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub pre_flight_commitment: RpcCommitment,
	pub encoding: Option<RpcBinaryEncoding>,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub min_context_slot: u64
}
//end-sendTransaction


// start-simulateTransaction
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionRequest {
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub sig_verify: bool,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub commitment: RpcCommitment,
	pub encoding: Option<RpcBinaryEncoding>,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub replace_recent_blockhash: bool,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub accounts: RpcSimulateTransactionRequestAccounts,
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub min_context_slot: u64
}
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionRequestAccounts {
	pub encoding: RpcBinaryEncoding,
	pub addresses: Vec<String>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponse {
	pub context: RpcResponseContext,
	pub value: RpcSimulateTransactionResponseValue
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseValue {
	pub err: Option<solana_sdk::transaction::TransactionError>,
	pub logs: Option<Vec<String>>,
	pub accounts: Option<Vec<RpcSimulateTransactionResponseAccounts>>,
	pub units_consumed: Option<u64>,
	pub return_data: Option<RpcSimulateTransactionResponseReturnData>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseAccounts {
	pub lamports: u64,
	pub owner: String,
	pub data: RPCBinaryEncodedString,
	pub executable: bool,
	pub rent_epoch: u64
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResponseReturnData {
	pub program_id: String,
	pub data: RPCBinaryEncodedString
}
// end-simulateTransaction


// start-signatureSubscribe
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureSubscribeResponse {
	pub context: RpcResponseContext,
	pub value: RpcSignatureSubscribeResponseValue
}
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureSubscribeResponseValue {
	pub err: Option<TransactionError>
}
// start-signatureSubscribe



// start-getSignatureStatusesRequest
#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetSignatureStatusesRequest {
	#[serde(default)]
	#[serde_as(deserialize_as = "DefaultOnNull")]
	pub search_transaction_history: bool
}


#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetSignatureStatusesResponse {
	pub context: RpcResponseContext,
	pub value: Vec<Option<RpcGetSignatureStatusesResponseValue>>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetSignatureStatusesResponseValue {
	pub slot: u64,
	pub confirmations: Option<usize>,
	pub confirmation_status: RpcCommitment,
	pub err: Option<solana_sdk::transaction::TransactionError>,
	pub status: Option<solana_sdk::transaction::TransactionError>,
}

// end-getSignatureStatusesRequest

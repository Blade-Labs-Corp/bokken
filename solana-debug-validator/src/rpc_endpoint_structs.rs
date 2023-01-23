

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

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct RPCBinaryEncodedString (String, RpcBinaryEncoding);
impl RPCBinaryEncodedString {
	pub fn from_bytes(data: &[u8], encoding: RpcBinaryEncoding) -> Self {
		Self(
			match &encoding {
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
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetAccountInfoRequest {
	#[serde(default)]
	pub commitment: RpcCommitment,
	#[serde(default)]
	pub encoding: RpcBinaryEncoding,
	pub data_slice: Option<RpcDataSlice>,
	#[serde(default)]
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
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcGetBalanceRequest {
	#[serde(default)]
	pub commitment: RpcCommitment,
	#[serde(default)]
	pub min_context_slot: u64
}

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
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSendTransactionRequest {
	#[serde(default)]
	pub skip_verify: bool,
	#[serde(default)]
	pub pre_flight_commitment: RpcCommitment,
	pub encoding: Option<RpcBinaryEncoding>,
	#[serde(default)]
	pub min_context_slot: u64
}
//end-sendTransaction


// start-simulateTransaction
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionRequest {
	#[serde(default)]
	pub sig_verify: bool,
	#[serde(default)]
	pub commitment: RpcCommitment,
	pub encoding: Option<RpcBinaryEncoding>,
	#[serde(default)]
	pub replace_recent_blockhash: bool,
	#[serde(default)]
	pub accounts: RpcSimulateTransactionRequestAccounts,
	#[serde(default)]
	pub min_context_slot: u64
}
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

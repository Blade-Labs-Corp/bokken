use std::{path::PathBuf, io, mem::size_of};

use borsh::{BorshSerialize, BorshDeserialize};
use bytemuck::{Zeroable, Pod};
use solana_sdk::{pubkey::Pubkey, transaction::{Transaction, TransactionError}, signature::Signature, program::MAX_RETURN_DATA};
use tokio::fs;

use crate::{error::BokkenDetailedError, utils::indexable_file::IndexableFile};

const MAX_TRANSACTION_SIZE: usize = 1232;
const DEFAULT_MAX_LOG_SIZE: usize = 50 * 1000; // 5 times more than original

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct BokkenLedgerFileHeader {
	rent_per_byte_year: u64,
	_unused: u64
}
impl Default for BokkenLedgerFileHeader {
	fn default() -> Self {
		Self { rent_per_byte_year: 3480, _unused: 0 }
	}
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
struct BokkenLedgerFileSlotEntryRaw {
	// Currently these are the same value, 
	slot: u64,
	block_height: u64,
	timestamp: i64,
	block_hash: [u8; 32],
	// Currently there's 1 transaction per block
	tx_data: Vec<u8>, // Transaction (legacy) w/ bincode
	tx_error: Vec<u8>, // TransactionError w/ bincode
	tx_return_data: Option<(Pubkey, Vec<u8>)>,
	tx_logs: Vec<String>
}
#[derive(Debug)]
pub struct BokkenLedgerFileSlotEntry {
	pub slot: u64,
	pub block_height: u64,
	pub timestamp: i64,
	pub block_hash: [u8; 32],
	// Currently there's 1 transaction per block
	pub tx_data: Transaction, // Transaction (legacy) w/ bincode
	pub tx_error: Option<TransactionError>, // TransactionError w/ bincode
	pub tx_return_data: Option<(Pubkey, Vec<u8>)>,
	pub tx_logs: Vec<String>
}
impl From<BokkenLedgerFileSlotEntryRaw> for BokkenLedgerFileSlotEntry {
    fn from(value: BokkenLedgerFileSlotEntryRaw) -> Self {
        Self {
			slot: value.slot,
			block_height: value.block_height,
			timestamp: value.timestamp,
			block_hash: value.block_hash,
			tx_data: bincode::deserialize(&value.tx_data).expect("tx_data deserialization"),
			tx_error: if value.tx_error.len() == 0 {
				None
			}else{
				Some(bincode::deserialize(&value.tx_error).expect("tx_error deserialization"))
			},
			tx_return_data: value.tx_return_data,
			tx_logs: value.tx_logs
		}
    }
}
impl From<BokkenLedgerFileSlotEntry> for BokkenLedgerFileSlotEntryRaw {
    fn from(value: BokkenLedgerFileSlotEntry) -> Self {
        Self {
			slot: value.slot,
			block_height: value.block_height,
			timestamp: value.timestamp,
			block_hash: value.block_hash,
			tx_data: bincode::serialize(&value.tx_data).expect("tx_sig deserialization"),
			tx_error: if let Some(tx_error) = value.tx_error {
				bincode::serialize(&tx_error).expect("tx_error deserialization")
			}else{
				Vec::new()
			},
			tx_return_data: value.tx_return_data,
			tx_logs: value.tx_logs
		}
    }
}

const LOG_TRUNCATED_MSG: &str = "Log truncated";
/// Global state for the Bokken ledger
#[derive(Debug)]
pub struct BokkenLedgerFile {
	slot: u64,
	blockhash: [u8; 32],
	rent_per_byte_year: u64,
	indexed_file_ref: IndexableFile<16, 8, u64, BokkenLedgerFileSlotEntryRaw>
}
impl BokkenLedgerFile {
	pub async fn new(path: PathBuf) -> Result<Self, color_eyre::eyre::Error> {
		let mut indexed_file_ref: IndexableFile<16, 8, u64, BokkenLedgerFileSlotEntryRaw> = IndexableFile::new(
			path,
			size_of::<u64>() + // slot
			size_of::<u64>() +
			size_of::<u64>() + 
			32 +
			64 + 
			MAX_TRANSACTION_SIZE + 4 +
			size_of::<TransactionError>() + 1 +
			size_of::<Pubkey>() + MAX_RETURN_DATA + 4 + 1 +
			DEFAULT_MAX_LOG_SIZE + 4,
			false
		).await?;
		
		let rent_per_byte_year;
		if let Some(header) = indexed_file_ref.read_file_header().await? {
			let header: &BokkenLedgerFileHeader = bytemuck::from_bytes(&header);
			rent_per_byte_year = header.rent_per_byte_year;
		}else{
			let header = BokkenLedgerFileHeader::default();
			rent_per_byte_year = header.rent_per_byte_year;
			indexed_file_ref.write_file_header(
				bytemuck::bytes_of(&header).try_into().unwrap()
			).await?;
		}
		if let Some((_, last_entry)) = indexed_file_ref.last().await? {
			Ok(
				Self {
					slot: last_entry.slot,
					blockhash: last_entry.block_hash,
					rent_per_byte_year,
					indexed_file_ref
				}
			)
		}else{
			Ok(
				Self {
					slot: 0,
					blockhash: <[u8; 32]>::default(),
					rent_per_byte_year,
					indexed_file_ref
				}
			)
		}
		
	}
	pub async fn read_block_at_slot(
		&self,
		slot: u64
	) -> Result<Option<BokkenLedgerFileSlotEntry> , BokkenDetailedError>{
		println!("DEBUG: read_block_at_slot({})", slot);
		let raw_result = self.indexed_file_ref.get(&slot).await?;
		if raw_result.is_some() {
			println!("DEBUG: read_block_at_slot({}): found!", slot);
		}else{
			println!("DEBUG: read_block_at_slot({}): not found", slot);
		}
		Ok(
			raw_result.map(|entry| {entry.into()})
		)
	}
	pub async fn append_new_block(
		&mut self,
		timestamp: i64,
		tx_data: Transaction, // Transaction (legacy) w/ bincode
		tx_error: Option<TransactionError>, // TransactionError w/ bincode
		tx_return_data: Option<(Pubkey, Vec<u8>)>,
		tx_logs: Vec<String>
	) -> Result<(), BokkenDetailedError> {
		let new_slot = self.slot + 1;
		let new_blockhash = {
			// We're not actually doing anything here yet, pass a fake value so things work
			let mut new_blockhash = <[u8; 32]>::default();
			new_blockhash[0..8].copy_from_slice(&self.slot.to_le_bytes());
			new_blockhash
		};
		let mut total_log_len = 0;
		let mut new_logs = Vec::new();
		for log in tx_logs {
			if (total_log_len + 4 + log.len() + LOG_TRUNCATED_MSG.len()) > DEFAULT_MAX_LOG_SIZE {
				new_logs.push(LOG_TRUNCATED_MSG.to_string());
				break;
			}
			total_log_len += log.len();
			new_logs.push(log);
		}
		self.indexed_file_ref.append(
			&new_slot,
			BokkenLedgerFileSlotEntry {
				slot: new_slot,
				block_height: new_slot,
				timestamp,
				block_hash: new_blockhash,
				tx_data,
				tx_error,
				tx_return_data,
				tx_logs: new_logs,
			}.into()
		).await?;
		self.slot = new_slot;
		// We're not doing anything with these for now. Use fake data so it still works
		self.blockhash[0..8].copy_from_slice(&new_slot.to_le_bytes());
		Ok(())
	}
	pub fn slot(&self) -> u64 {
		self.slot
	}
	pub fn blockhash(&self) -> [u8; 32] {
		self.blockhash
	}
	pub fn rent_per_byte_year(&self) -> u64 {
		self.rent_per_byte_year
	}
}

use std::{path::PathBuf, collections::{HashMap, HashSet}, io, time::{SystemTime, UNIX_EPOCH}};

use borsh::{BorshSerialize, BorshDeserialize};
use color_eyre::eyre;
use bokken_runtime::debug_env::{BokkenAccountData, BorshAccountMeta};
use solana_sdk::{pubkey, pubkey::Pubkey, system_program, transaction::{TransactionError, Transaction}};
use tokio::fs;
use lazy_static::lazy_static;

mod ledger_file;

use crate::{error::{BokkenError, BokkenDetailedError}, program_caller::ProgramCaller, debug_ledger::ledger_file::BokkenLedgerFile, utils::indexable_file::IndexableFile};

use self::ledger_file::BokkenLedgerFileSlotEntry;

const RENT_BASE_SIZE: u64 = 128;
pub const PUBKEY_NULL: Pubkey = pubkey!("nu11111111111111111111111111111111111111111");
pub const PUBKEY_DEBUG_PROGRAM_LOADER: Pubkey = pubkey!("Debugab1eProgramLoader111111111111111111111");
lazy_static! {
    static ref GHOST_DATA: Vec<u8> = vec![0xf0, 0x9f, 0x91, 0xbb, 0xf0, 0x9f, 0x90, 0x9b, 0xf0, 0x9f, 0xa7, 0x91, 0xe2, 0x80, 0x8d, 0xf0, 0x9f, 0x92, 0xbb];
}

#[derive(Debug)]
pub struct BokkenLedgerInitConfig {
	pub initial_mint: Pubkey,
	pub initial_mint_lamports: u64
}

/// Abstraction around Bokken's save directory
#[derive(Debug)]
pub struct BokkenLedger {
	base_path: PathBuf,
	accounts_path: PathBuf,
	program_caller: ProgramCaller,
	transaction_index: IndexableFile<0, 64, [u8; 64], u64>,
	state: BokkenLedgerFile
}
#[derive(Debug)]
pub struct BokkenLedgerInstruction {
	pub program_id: Pubkey,
	pub account_metas: Vec<BorshAccountMeta>,
	pub data: Vec<u8>
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BokkenLedgerAccountReturnChoice {
	None,
	All,
	Edited,
	Only(Vec<Pubkey>)
}
impl BokkenLedger {
	/// Manages Bokken's state at the specified path
	/// 
	pub async fn new(
		base_path: PathBuf,
		program_caller: ProgramCaller,
		init_mint_config: Option<BokkenLedgerInitConfig>
	) -> eyre::Result<Self> {
		let accounts_path = {
			let mut p = base_path.clone();
			p.push("accounts");
			p
		};
		let state_path = {
			let mut p = base_path.clone();
			p.push("state.blob");
			p
		};
		let tx_index_path = {
			let mut p = base_path.clone();
			p.push("state_tx_index.blob");
			p
		};
		let create_initial_mint = match fs::create_dir(&base_path).await {
			Ok(_) => {
				fs::create_dir(&accounts_path).await?;
				true
			},
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				// TODO: Verify integrity?
				false
			},
			Err(e) => {
				return Err(e.into())
			}
		};
		let new_self = Self {
			base_path,
			accounts_path,
			program_caller,
			state: BokkenLedgerFile::new(state_path).await?,
			transaction_index: IndexableFile::new(
				tx_index_path,
				8,
				true
			).await?
		};
		if create_initial_mint {
			let init_mint_config = init_mint_config.ok_or(BokkenError::InitConfigIsNone)?;
			let init_mint_account = BokkenAccountData {
				lamports: init_mint_config.initial_mint_lamports,
				data: Vec::new(),
				owner: system_program::id(),
				executable: false,
				rent_epoch: 0
			};
			new_self.save_account(&init_mint_config.initial_mint, &init_mint_account).await?;
			println!("Created initial mint @ {}", init_mint_config.initial_mint);
		}
		Ok(new_self)
	}
	pub fn slot(&self) -> u64 {
		self.state.slot()
	}
	pub fn blockhash(&self) -> [u8; 32] {
		self.state.blockhash()
	}
	pub fn calc_min_balance_for_rent_exemption(&self, data_len: u64) -> u64 {
		(RENT_BASE_SIZE + data_len) * self.state.rent_per_byte_year() * 2
	}
	pub async fn get_bokken_entry_by_tx(&self, tx_sig: [u8; 64]) -> Result<Option<BokkenLedgerFileSlotEntry>, BokkenDetailedError> {
		if let Some(tx_slot) = self.transaction_index.get(&tx_sig).await? {
			return Ok(
				self.state.read_block_at_slot(tx_slot).await?
			);
		}
		Ok(None)
	}
	pub async fn save_account(&self, pubkey: &Pubkey, data: &BokkenAccountData) -> Result<(), BokkenDetailedError> {
		// TODO: This is terrible, replace with IndexableFile
		let mut account_path = self.accounts_path.clone();
		account_path.push(pubkey.to_string());
		fs::create_dir_all(&account_path).await?;
		account_path.push(self.slot().to_string());
		fs::write(
			&account_path,
			if data.lamports == 0 {
				BokkenAccountData::default().try_to_vec()?
			}else{
				data.try_to_vec()?
			}
		).await?;
		Ok(())
	}
	pub async fn read_account(
		&self,
		pubkey: &Pubkey,
		clock_time_override_hack: Option<(u64, i64)>
	) -> Result<BokkenAccountData, BokkenError> {
		if self.program_caller.has_program_id(pubkey).await {
			return Ok(
				BokkenAccountData {
					lamports: 0xf09f91bb,
					data: GHOST_DATA.clone(),
					owner: PUBKEY_DEBUG_PROGRAM_LOADER,
					executable: true,
					rent_epoch: 0
				}
			)
		}

		// TODO: This is terrible
		if *pubkey == solana_sdk::sysvar::clock::id() {
			let (slot, unix_timestamp) = clock_time_override_hack.unwrap_or_else(||{
				(
					self.slot(),
					SystemTime::now().duration_since(UNIX_EPOCH).expect("We're in 1970").as_secs() as i64
				)
			});
			return Ok(
				BokkenAccountData {
					lamports: 0xf09f91bb,
					data: bincode::serialize(
						&solana_sdk::sysvar::clock::Clock {
							slot,
							epoch_start_timestamp: 0,
							epoch: 0,
							leader_schedule_epoch: 0,
							unix_timestamp
						}
					).expect("clock sysvar couln't be serialized"),
					owner: pubkey!("Sysvar1111111111111111111111111111111111111"),
					executable: false,
					rent_epoch: 0
				}
			)
		}
		
		if *pubkey == solana_sdk::sysvar::rent::id() {
			return Ok(
				BokkenAccountData {
					lamports: 0xf09f91bb,
					data: bincode::serialize(
						&solana_sdk::sysvar::rent::Rent {
							lamports_per_byte_year: self.state.rent_per_byte_year(),
							exemption_threshold: 2.0,
							burn_percent: 100 // we don't have no "validators" here
						}
					).expect("Rent sysvar couln't be serialized"),
					owner: pubkey!("Sysvar1111111111111111111111111111111111111"),
					executable: false,
					rent_epoch: 0
				}
			)
		}

		let mut account_path = self.accounts_path.clone();
		account_path.push(pubkey.to_string());
		
		// TODO: This is terrible, replace with IndexableFile
		match fs::read_dir(&account_path).await {
			Ok(mut files) => {
				let mut max_slot = 0u64;
				while let Some(file) = files.next_entry().await? {
					let slot = file.file_name().to_str().unwrap_or_default().parse::<u64>().unwrap_or_default();
					if slot > max_slot {
						max_slot = slot;
					}
				}
				account_path.push(max_slot.to_string());
				match fs::read(account_path).await {
					Ok(file_data) => {
						let file_data_parsed = BokkenAccountData::try_from_slice(&file_data)?;
						Ok(file_data_parsed)
					},
					Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
						Ok(BokkenAccountData::default())
					},
					Err(e) => {
						return Err(e.into())
					}
				}
			},
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				Ok(BokkenAccountData::default())
			},
			Err(e) => {
				return Err(e.into())
			}
		}
	}
	async fn execute_instruction(
		&mut self,
		instruction: BokkenLedgerInstruction,
		call_depth: u8,
		state: &mut HashMap<Pubkey, BokkenAccountData>
	) -> Result<(u64, Vec<String>), BokkenDetailedError> {
		// Only send ixs required to the child process (this probably wastes more perf than it saves)
		let account_datas_for_ix = {
		 	let mut account_datas_for_ix = HashMap::new();
			// Insert rent sysvar
			account_datas_for_ix.insert(
				solana_sdk::sysvar::rent::id(),
				state.get(&solana_sdk::sysvar::rent::id()).unwrap().clone()
			);
			// insert clock sysvar
			account_datas_for_ix.insert(
				solana_sdk::sysvar::clock::id(),
				state.get(&solana_sdk::sysvar::clock::id()).unwrap().clone()
			);
			for meta in instruction.account_metas.iter() {
				if !account_datas_for_ix.contains_key(&meta.pubkey) {
					account_datas_for_ix.insert(
						meta.pubkey.clone(),
						state.remove(&meta.pubkey).ok_or(TransactionError::AccountNotFound)?
					);
				}
			}
			account_datas_for_ix
		};

		let (return_code, logs, account_datas_for_ix) = self.program_caller.call_program(
			instruction.program_id,
			instruction.data,
			instruction.account_metas,
			account_datas_for_ix,
			call_depth
		).await?;

		// do stuff
		for (pubkey, account_data) in account_datas_for_ix.into_iter() {
			// re-insert edited state back in
			state.insert(pubkey, account_data);
		}
		Ok((return_code, logs))
	}
	pub async fn execute_transaction(
		&mut self,
		tx: Transaction,
		commit_changes: bool
	) -> Result<(), BokkenDetailedError> {
		let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("We're in 1970").as_secs() as i64;
		let new_slot = self.slot() + 1;

		let account_pubkeys = &tx.message.account_keys;
		let ixs: Vec<BokkenLedgerInstruction> = tx.message.instructions.iter().map(|ix| {
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
		let (_, logs) = self.execute_instructions(
			&tx.message.account_keys[0],
			ixs,
			BokkenLedgerAccountReturnChoice::None,
			Some((new_slot, cur_time)),
			commit_changes
		).await?;
		//tx.signatures[0]
		if commit_changes {
			self.transaction_index.insert(&tx.signatures[0].into(), new_slot).await?;
			self.state.append_new_block(
				cur_time,
				tx,
				// We simply don't save txs with errors for now
				None,
				// We're not getting return data from the child process yet
				None,
				logs
			).await?;
		}
		
		Ok(())
	}


	/// Execute the specified data as a transaction instruction
	/// Saves any changes and increments the block slot if `commit_changes` is true
	pub async fn execute_instructions(
		&mut self,
		fee_payer: &Pubkey,
		instructions: Vec<BokkenLedgerInstruction>,
		return_choice: BokkenLedgerAccountReturnChoice,
		clock_time_override_hack: Option<(u64, i64)>,
		commit_changes: bool
	) -> Result<(HashMap<Pubkey, BokkenAccountData>, Vec<String>), BokkenDetailedError> {
		let mut the_big_log = Vec::new();
		let mut unique_sigs = HashSet::new();
		unique_sigs.insert(fee_payer.clone()); //
		let account_datas = {
			let mut account_datas = HashMap::new();
			// Fee payer
			account_datas.insert(fee_payer.clone(), self.read_account(fee_payer, clock_time_override_hack).await?);
			// rent sysvar (needed for Rent::get to work)
			account_datas.insert(
				solana_sdk::sysvar::rent::id(),
				self.read_account(&solana_sdk::sysvar::rent::id(), clock_time_override_hack).await?
			);
			// clock sysvar (needed for Clock::get to work)
			account_datas.insert(
				solana_sdk::sysvar::clock::id(),
				self.read_account(&solana_sdk::sysvar::clock::id(), clock_time_override_hack).await?
			);
			for ix in instructions.iter() {
				for meta in ix.account_metas.iter() {
					if meta.is_signer {
						unique_sigs.insert(meta.pubkey.clone());
					}
					if !account_datas.contains_key(&meta.pubkey) {
						account_datas.insert(meta.pubkey, self.read_account(&meta.pubkey, clock_time_override_hack).await?);
					}
				}
			}
			account_datas
		};
		let mut account_datas_changed = account_datas.clone();
		{
			// Take the fee away!
			let fee_payer = account_datas_changed.get_mut(fee_payer)
				.expect("For the fee payer data to be where we put it");
			// sig fee is hard-coded for now
			// TODO: care about about the 128 bytes for rent
			fee_payer.lamports = fee_payer.lamports.checked_sub(
				5000 * unique_sigs.len() as u64
			).ok_or(TransactionError::InsufficientFundsForFee)?;
			// fee_payer gets dropped
		}

		for (i, ix) in instructions.into_iter().enumerate() {
			let (return_code, logs) = self.execute_instruction(ix, 1, &mut account_datas_changed).await?;
			the_big_log.extend(logs);
			if return_code != 0 {
				return Err(BokkenError::InstructionExecError(i, return_code.into(), the_big_log).into());
			}
		}
		let edited_accounts = {
			let mut result = HashMap::new();
			for (pubkey, old_data) in account_datas.into_iter() {
				let new_data = account_datas_changed.get(&pubkey).unwrap().clone();
				if new_data != old_data {
					if commit_changes {
						self.save_account(&pubkey, &new_data).await?;
					}
					result.insert(pubkey, new_data);
				}
			}
			result
		};
		let account_data_result = match return_choice {
			BokkenLedgerAccountReturnChoice::None => {
				HashMap::new()
			}
			BokkenLedgerAccountReturnChoice::All => {
				account_datas_changed
			},
			BokkenLedgerAccountReturnChoice::Edited => {
				edited_accounts
			},
			BokkenLedgerAccountReturnChoice::Only(pubkeys) => {
				let mut result = HashMap::new();
				for pubkey in pubkeys.into_iter() {
					result.insert(pubkey, account_datas_changed.get(&pubkey).unwrap().clone());
				}
				result
			}
		};
		Ok((account_data_result, the_big_log))
	}
}

use std::{cmp::Ordering::{Greater, Less}, marker::PhantomData, io::SeekFrom, path::Path};

use borsh::{BorshDeserialize, BorshSerialize};
use tokio::{io::{AsyncSeekExt, AsyncReadExt, AsyncWriteExt}, sync::Mutex, fs};

use crate::error::{BokkenDetailedError, BokkenError};



/// A file you can search through.
/// Ensures that the data cannot be edited while it is being written.
/// Functions like a sorted map, duplicate keys will be overwritten
struct IndexableFile<
	const IDENTIFIER_SIZE: usize,
	I: Ord + BorshDeserialize + BorshSerialize,
	T: BorshDeserialize + BorshSerialize
> {
	file_ref: Mutex<fs::File>,
	file_len: u64,
	begin_offset: usize,
	identifier_type: PhantomData<I>,
	entry_size: usize,
	entry_type: PhantomData<T>,
	indentifier_is_seperate_from_entry: bool,
	
}
impl<
	const IDENTIFIER_SIZE: usize,
	I: Ord + BorshDeserialize + BorshSerialize,
	T: BorshDeserialize + BorshSerialize
> IndexableFile<IDENTIFIER_SIZE, I, T> {
	async fn new(
		path: impl AsRef<Path>,
		begin_offset: usize,
		entry_size: usize,
		indentifier_is_seperate_from_entry: bool
	) -> Result<Self, BokkenDetailedError> {
		let file_ref = fs::OpenOptions::new()
			.read(true)
			.write(true)
			.create(true)
			.truncate(true)
			.open(path).await?;
		let file_metadata = file_ref.metadata().await?;
		Ok(
			Self {
				file_ref: Mutex::new(file_ref),
				file_len: file_metadata.len(),
				begin_offset,
				identifier_type: PhantomData,
				entry_size,
				entry_type: PhantomData,
				indentifier_is_seperate_from_entry
			}
		)
	}
	fn _index_to_offset(&self, index: usize) -> u64 {
		self.begin_offset as u64 +
			index as u64 *
			(
				IDENTIFIER_SIZE as u64 *
				self.indentifier_is_seperate_from_entry as u64 +
				self.entry_size as u64
			)
	}
	pub fn len(&self) -> usize {
		let result = (
			self.file_len - self.begin_offset as u64
		) / (
			IDENTIFIER_SIZE as u64 *
			self.indentifier_is_seperate_from_entry as u64 +
			self.entry_size as u64
		);
		return result.try_into().expect("max entries to not exceed usize");
	}
	fn _check_index(
		&self,
		index: usize
	) -> Result<(), BokkenError> {
		let len = self.len();
		if index >= len {
			return Err(BokkenError::IndexFileOutOfBounds(index, len))
		}
		Ok(())
	}
	async fn _read_identifier_at_index(
		&self,
		index: usize,
		file_ref: &mut fs::File
	) -> Result<I, BokkenDetailedError> {
		file_ref.seek(SeekFrom::Start(self._index_to_offset(index))).await?;
		let mut identifier_bytes = [0u8; IDENTIFIER_SIZE];
		let data_read = file_ref.read(&mut identifier_bytes).await?;
		if data_read < IDENTIFIER_SIZE {
			return Err(BokkenError::UnexpectedEOF.into());
		}
		// Doing it this way because we might not even read the entire buffer
		Ok(
			I::deserialize(&mut identifier_bytes.as_slice())?
		)
	}
	async fn _read_entry_at_index(
		&self,
		index: usize,
		file_ref: &mut fs::File
	) -> Result<T, BokkenDetailedError> {
		file_ref.seek(SeekFrom::Start(
			self._index_to_offset(index) + (
				IDENTIFIER_SIZE as u64 * self.indentifier_is_seperate_from_entry as u64
			)
		)).await?;
		let mut entry_bytes = vec![0u8; self.entry_size];
		let data_read = file_ref.read(&mut entry_bytes).await?;
		if data_read < self.entry_size {
			return Err(BokkenError::UnexpectedEOF.into());
		}
		// Doing it this way because we might not even read the entire buffer
		Ok(
			T::deserialize(&mut entry_bytes.as_slice())?
		)
	}
	async fn _binary_search(
		&self,
		x: &I,
		file_ref: &mut fs::File
	) -> Result<IndexableFileSearchResult, BokkenDetailedError> {
		// Code stolen from core::slice::binary_search_by
		let mut size = self.len();
		let mut left = 0;
		let mut right = size;
		
		while left < right {
			let mid = left + size / 2;
			let cmp = self._read_identifier_at_index(mid, file_ref).await?.cmp(x);

			if cmp == Less {
				left = mid + 1;
			} else if cmp == Greater {
				right = mid;
			} else {
				return Ok(IndexableFileSearchResult::Found(mid));
			}
			size = right - left;
		}
		
		Ok(IndexableFileSearchResult::NotFound(left))
	}
	pub async fn first(&self) -> Result<Option<(I, T)>, BokkenDetailedError> {
		if self.len() == 0 {
			return Ok(None);
		}
		let file_ref = &mut self.file_ref.lock().await;
		Ok(Some((
			self._read_identifier_at_index(0, file_ref).await?,
			self._read_entry_at_index(0, file_ref).await?
		)))
	}
	pub async fn last(&self) -> Result<Option<(I, T)>, BokkenDetailedError> {
		let mut index = self.len();
		if index == 0 {
			return Ok(None);
		}
		index -= 1;
		let file_ref = &mut self.file_ref.lock().await;
		Ok(Some((
			self._read_identifier_at_index(index, file_ref).await?,
			self._read_entry_at_index(index, file_ref).await?
		)))
	}
	pub async fn get(&self, key: &I) -> Result<Option<T>, BokkenDetailedError> {
		let file_ref = &mut self.file_ref.lock().await;
		match self._binary_search(key, file_ref).await? {
			IndexableFileSearchResult::Found(index) => {
				Ok(
					Some(
						self._read_entry_at_index(index, file_ref).await?
					)
				)
			},
			IndexableFileSearchResult::NotFound(_) => {
				Ok(None)
			},
		}
	}
	pub async fn insert(&mut self, key: &I, value: T) -> Result<Option<T>, BokkenDetailedError> {
		let file_ref = &mut self.file_ref.lock().await;
		let (index, old_value) = match self._binary_search(key, file_ref).await? {
			IndexableFileSearchResult::Found(index) => {
				(index, Some(self._read_entry_at_index(index, file_ref).await?))
			},
			IndexableFileSearchResult::NotFound(index) => {
				(index, None)
			},
		};
		if old_value.is_none() {
			let old_len = self.len();
			let new_file_len = self._index_to_offset(old_len + 1);
			file_ref.set_len(new_file_len).await?;
			self.file_len = new_file_len;
			let mut tmp_entry_bytes = vec![
				0u8;
				self.entry_size + IDENTIFIER_SIZE * self.indentifier_is_seperate_from_entry as usize
			];
			for i in (0..old_len).rev() {
				file_ref.seek(SeekFrom::Start(self._index_to_offset(i))).await?;
				file_ref.read(&mut tmp_entry_bytes.as_mut_slice()).await?;
				file_ref.seek(SeekFrom::Start(self._index_to_offset(i + 1))).await?;
				file_ref.write(&tmp_entry_bytes).await?;
			}
		}
		let mut entry_bytes = vec![0u8; self.entry_size];
		value.serialize(&mut entry_bytes.as_mut_slice())?;
		if entry_bytes.len() != self.entry_size {
			unreachable!("entry serialization was done wrong");
		}
		file_ref.seek(SeekFrom::Start(self._index_to_offset(index))).await?;
		if self.indentifier_is_seperate_from_entry {
			let mut identifier_bytes = [0u8; IDENTIFIER_SIZE];
			key.serialize(&mut identifier_bytes.as_mut_slice())?;
			file_ref.write(&identifier_bytes).await?;
		}
		file_ref.write(&entry_bytes).await?;
		Ok(old_value)
	}
	pub async fn append(&mut self, key: &I, value: T) -> Result<(()), BokkenDetailedError> {
		let file_ref = &mut self.file_ref.lock().await;
		let old_len = self.len();
		if old_len > 0 && *key <= self._read_identifier_at_index(old_len - 1, file_ref).await? {
			return Err(BokkenError::CannotAppendToIndex.into());
		}
		let new_file_len = self._index_to_offset(old_len + 1);
		file_ref.set_len(new_file_len).await?;
		self.file_len = new_file_len;
		file_ref.seek(SeekFrom::Start(self._index_to_offset(old_len))).await?;
		let mut entry_bytes = vec![0u8; self.entry_size];
		value.serialize(&mut entry_bytes.as_mut_slice())?;
		if entry_bytes.len() != self.entry_size {
			unreachable!("entry serialization was done wrong");
		}
		if self.indentifier_is_seperate_from_entry {
			let mut identifier_bytes = [0u8; IDENTIFIER_SIZE];
			key.serialize(&mut identifier_bytes.as_mut_slice())?;
			file_ref.write(&identifier_bytes).await?;
		}
		file_ref.write(&entry_bytes).await?;
		Ok(())
	}
}

#[derive(Debug)]
pub enum IndexableFileSearchResult {
	Found(usize),
	NotFound(usize)
}

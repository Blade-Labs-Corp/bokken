use std::{collections::{VecDeque}, io, sync::{Arc, atomic::{AtomicBool, Ordering}}};

use borsh::{BorshSerialize, BorshDeserialize};
// use borsh::{BorshSerialize, BorshDeserialize};
use tokio::{task, net::{UnixStream, unix}, sync::{Mutex, watch}};


enum IPCCommReadState {
	MsgLength,
	MsgBody
}
enum IPCCommReadResult {
	Shutdown,
	Waiting,
	Message(Vec<u8>)
}


struct IPCCommReadHandler {
	buffer: Vec<u8>,
	buffer_index: usize,
	state: IPCCommReadState,
	stream: unix::OwnedReadHalf
}
impl IPCCommReadHandler {
	pub fn new(
		stream: unix::OwnedReadHalf,
	) -> Self {
		Self {
			buffer: vec![0; 8],
			buffer_index: 0,
			state: IPCCommReadState::MsgLength,
			stream
		}
	}
	async fn read_tick(&mut self) -> Result<IPCCommReadResult, io::Error> {
		self.stream.readable().await?;

		
		let buf_slice = &mut self.buffer.as_mut_slice()[self.buffer_index..];
		if buf_slice.len() == 0 {
			panic!("Zero-length message, this shouldn't happen");
		}
		let read_result = match self.stream.try_read(buf_slice) {
			Ok(0) => {
				IPCCommReadResult::Shutdown
			},
			Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
				IPCCommReadResult::Waiting
			},
			Ok(n) => {
				self.buffer_index += n;
				if self.buffer_index == self.buffer.len() {
					match self.state {
						IPCCommReadState::MsgLength => {
							let size = u64::from_le_bytes(
								self.buffer.as_slice()
									.try_into()
									.expect("vector for msg len should have been 8 bytes long")
							);
							self.buffer = vec![0; size as usize];
							self.buffer_index = 0;
							self.state = IPCCommReadState::MsgBody;
							IPCCommReadResult::Waiting
						},
						IPCCommReadState::MsgBody => {
							let final_msg = self.buffer.clone();
							self.buffer = vec![0; 8];
							self.buffer_index = 0;
							self.state = IPCCommReadState::MsgLength;
							IPCCommReadResult::Message(final_msg)
						}
					}
				}else{
					IPCCommReadResult::Waiting
				}
			}
			Err(e) => {
				return Err(e.into())
			}
		};
		Ok(read_result)
	}
}


struct IPCCommWriteHandler {
	queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
	stream: unix::OwnedWriteHalf
}
impl IPCCommWriteHandler {
	pub fn new(
		stream: unix::OwnedWriteHalf,
		bytes_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
	) -> Self {
		Self {
			queue: bytes_queue,
			stream
		}
	}
	async fn write_tick(&mut self) -> Result<(), io::Error> {
		self.stream.writable().await?;
		let mut send_queue = self.queue.lock().await;
		if let Some(send_data) = send_queue.pop_front() {
			match self.stream.try_write(send_data.as_slice()) {
				Ok(n) => {
					if n < send_data.len() {
						// Not all the bytes have been written, add the remaining ones to the queue
						send_queue.push_front(send_data[{send_data.len() - n}..].into());
					}
				},
				Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
					// We can't write it now, add it to the queue
					send_queue.push_front(send_data);
				},
				Err(e) => {
					return Err(e.into())
				}
			}
		}
		Ok(())
	}
}

// #[derive(Debug, Clone)]
#[derive(Debug)]
pub struct IPCComm {
	write_handle: task::JoinHandle<()>,
	read_handle: task::JoinHandle<()>,
	should_stop: Arc<AtomicBool>,
	send_queue_bytes: Arc<Mutex<VecDeque<Vec<u8>>>>,
	recv_queue_bytes: Arc<Mutex<VecDeque<Vec<u8>>>>,
	recv_notif: watch::Receiver<usize>
}

impl IPCComm {
	pub fn new(
		stream: UnixStream,
	) -> Self {
		// let send_queue = Arc::new(Mutex::new(VecDeque::new()));
		// et send_queue_clone = send_queue.clone();
		let recv_queue_bytes_mutex = Arc::new(Mutex::new(VecDeque::new()));
		let send_queue_bytes_mutex = Arc::new(Mutex::new(VecDeque::new()));
		let should_stop = Arc::new(AtomicBool::new(false));
		let (recv_notif_sender, recv_notif) = watch::channel(0usize);


		let (read_stream, write_stream) = stream.into_split();

		let mut read_handler = IPCCommReadHandler::new(read_stream);
		let should_stop_clone = should_stop.clone();
		let recv_queue_bytes_mutex_clone = recv_queue_bytes_mutex.clone();
		let read_handle = task::spawn(async move {
			while !should_stop_clone.load(Ordering::Relaxed) {
				match read_handler.read_tick().await.unwrap() {
					IPCCommReadResult::Shutdown => {
						should_stop_clone.store(true, Ordering::Relaxed);
						recv_notif_sender.send_modify(|val| {
							(*val, _) = val.overflowing_add(1)
						})
					},
					IPCCommReadResult::Waiting => {
						// Nothing else to do!
					},
					IPCCommReadResult::Message(msg_bytes) => {
						let mut recv_queue_bytes = recv_queue_bytes_mutex_clone.lock().await;
							recv_queue_bytes.push_back(msg_bytes);
						recv_notif_sender.send_modify(|val| {
							(*val, _) = val.overflowing_add(1)
						})
					},
				}
			}
		});

		let mut write_handler = IPCCommWriteHandler::new(write_stream, send_queue_bytes_mutex.clone());
		let should_stop_clone = should_stop.clone();
		let write_handle = task::spawn(async move {
			while !should_stop_clone.load(Ordering::Relaxed) {
				write_handler.write_tick().await.unwrap();
			}
		});
		
		Self {
			write_handle,
			read_handle,
			should_stop,
			send_queue_bytes: send_queue_bytes_mutex,
			recv_queue_bytes: recv_queue_bytes_mutex,
			recv_notif
		}
	}
	// Waits until I is recievied, will error if the initial message couldn't be decoded
	pub async fn new_with_identifier<I: BorshDeserialize>(stream: UnixStream) -> Result<(Self, I), io::Error> {
		let mut sayulf = Self::new(stream);
		let id = sayulf.until_recv_msg().await?.ok_or(io::Error::from(io::ErrorKind::UnexpectedEof))?;
		Ok((sayulf, id))
	}
	pub async fn send_msg<S: BorshSerialize>(&mut self, msg: S) -> Result<(), io::Error> {
		let msg_bytes = msg.try_to_vec()?;
		let mut send_queue_bytes = self.send_queue_bytes.lock().await;
		send_queue_bytes.push_back((msg_bytes.len() as u64).to_le_bytes().to_vec());
		send_queue_bytes.push_back(msg_bytes);
		Ok(())
	}
	pub fn blocking_send_msg<S: BorshSerialize>(&mut self, msg: S) -> Result<(), io::Error> {
		let msg_bytes = msg.try_to_vec()?;
		let mut send_queue_bytes = self.send_queue_bytes.blocking_lock();
		send_queue_bytes.push_back((msg_bytes.len() as u64).to_le_bytes().to_vec());
		send_queue_bytes.push_back(msg_bytes);
		Ok(())
	}
	/// Results in None if thereare no messages in the incoming message queue
	pub async fn recv_msg<R: BorshDeserialize>(&mut self) -> Result<Option<R>, io::Error> {
		let mut recv_queue_bytes = self.recv_queue_bytes.lock().await;
		match recv_queue_bytes.pop_front() {
			Some(msg_bytes) => {
				Ok(Some(R::try_from_slice(&msg_bytes)?))
			},
			None => Ok(None),
		}
	}
	// Wait until there's a message to be received. Results in None if the read stream has been shut down
	pub async fn until_recv_msg<R: BorshDeserialize>(&mut self) -> Result<Option<R>, io::Error> {
		loop {
			if self.should_stop.load(Ordering::Relaxed) {
				return Ok(None);
			}
			if let Some(msg) = self.recv_msg::<R>().await? {
				return Ok(Some(msg));
			}
			self.recv_notif.changed().await.expect("Recever shouldn't drop without sending a message first");
		}
	}
	pub fn stopped(&self) -> bool {
		self.should_stop.load(Ordering::Relaxed)
	}
	pub fn stop(&self) {
		self.should_stop.store(true, Ordering::Relaxed);
	}
	pub async fn wait_until_stopped(self) {
		self.write_handle.await.unwrap();
		self.read_handle.await.unwrap();
	}

}

#![cfg_attr(
	// Today (2022-11-03) I learned that the bpf sdk is provided by solana and is in fact behind upstream 🙃
    target_arch = "bpf",
    feature(backtrace)
)]
pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

pub use entrypoint::entrypoint;

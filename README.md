# Bokken

_A Solana program debugging tool_

* Ever wanted to add a breakpoint in your Solana program...
  * ...while having persistent state
  * ...and while calling it from JSONRPC
  * ...without modifying your program source?
* Do you want line numbers and stack traces in your errors?
* Does small a part of you die every time you re-deploy your program just because you didn't add enough `msg!` statements?

Well, with the magic of not compiling to SBF (Solana Bytecode Format), your Solana program debugging woes can be washed away!

[GIF Showing step-debugging through Solana Programs]

## Preamble

The goal of Bokken, like its namesake, is to have your programs practice their skills in a controlled environment before entering the real world. To achieve this goal, Bokken provides an emulated environment for your programs to execute in similar to the Truffle Suite for Ethereum.

This means you can write your integration tests in JS/TS using `@solana/web3.js` in order to test your Solana program and front-end code at the same time with instant-confirmation transactions!

## Features

This project is still in early development. Because of this, not all Solana features are currently implemented/emulated.

* Implemented functionality
  * Program logging
  * System Program emulation (create account, transfer, alloc, etc.)
  * Cross-program invocations
  * Persistent state
  * State rollback (partial)
  * Return data (partial)
  * `simulateTransaction`
  * `getAccountInfo`
  * `getBlockHeight`
  * `getLatestBlockhash` (With fake data)
  * `sendTransaction`
* Pending features
  * Anything req'd for `sendAndConfirmTransaction`
  * Lookups for "Transactions/Slots"
  * Sysvars
  * Importing state from pre-existing snapshots created with `solana-ledger-tool` (Mainnet-beta, pre-existing `solana-test-validator` ledgers, etc.)
  * Invoking pre-compiled SBF programs
  * Staking rewards emulation

## Usage

1. Create a new cargo project with the following dependencies. The `bokken-runtime` will use your program as a dependency and call its entrypoint function when executing transactions.
```toml
[dependencies]
bokken-runtime = "0.1"
your-program-crate-name-here = {path = "path/to/program/source"}
tokio = "1.0"
color-eyre = "0.5"
```
2. Use the provided macro from `bokken-runtime` to generate the main function for your debuggable program. Your `main.rs` should be similar to this:
```rs
bokken_runtime::bokken_program!(your_program_crate_name_here);
```
3. Confirm that your program compiles. The output should be similar to this
```
$ cargo run -- --help
A native-compiled Solana program to be used with Bokken

Usage: -s PATH -p PUBKEY

Available options:
    -s, --socket-path <PATH>   The unix socket of the Bokken instance to link to
    -p, --program-id <PUBKEY>  Program ID of this program
    -h, --help                 Prints help information
    -V, --version              Prints version information
```
4. Launch Bokken (Listens to 127.0.0.1:8899 by default)
```
bokken --socket-path /tmp/bokken.sock --save-path /tmp/bokken-data
```
5. Launch your program (from your newly-created project directory) and connect it to Bokken. The program ID can be any valid PublicKey.
```
cargo run -- --socket-path /tmp/bokken.sock --program-id YourAwesomeDebugab1eProgram1111111111111111
```
You should see a message saying "Registered new debugable program: YourAwesomeDebugab1eProgram1111111111111111 in Bokken's console"

Now you can send transactions to it to your hearts content!

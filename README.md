# Bokken: A Solana program debugging tool

_Finally, a way to actually stepping through your solana code with persistant state! Amazing!_

![A GIF showing a solana program being step-debugged with with a hilight on struct inspection](https://cdn.discordapp.com/attachments/898958435410915348/1067119876994519040/solana-step-debug-absent-theme.gif)

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

## How it works

At the time of writing, the only tool available for local solana program development is the `solana-test-validator`. However, there are 2 major limitations.
* It doesn't provide any mechanism for stepping through transactions (like ethereum's remix IDE for example)
* You cannot have stack traces, attempting to capture one will just output the string `<unsupported>`

In order to avoid these issues, the basic premise behind Bokken is to compile your Solana program to your system's native platform. This enables the use of capturing stack traces as well as the ability to use pre-existing Rust debugging tools, such as the VSCode CodeLLDB extension.

This, combined with the `solana_program` crate's ability to specify custom solana-sycall implementations when not compiled to SBF/BPF, allows you to debug your programs in an enviroment which can closesly resemble that during normal program execution.

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

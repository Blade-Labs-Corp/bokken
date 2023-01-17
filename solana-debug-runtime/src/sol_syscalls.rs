use solana_program::{program_stubs::SyscallStubs, program_error::UNSUPPORTED_SYSVAR, entrypoint::ProgramResult, pubkey::Pubkey, instruction::Instruction, account_info::AccountInfo};


struct DebugValidatorSyscalls {}

impl SyscallStubs for DebugValidatorSyscalls {
	fn sol_log(&self, message: &str) {
		println!("{}", message);
	}
	fn sol_log_compute_units(&self) {
		self.sol_log("SyscallStubs: sol_log_compute_units() not available");
	}
	fn sol_invoke_signed(
		&self,
		_instruction: &Instruction,
		_account_infos: &[AccountInfo],
		_signers_seeds: &[&[&[u8]]],
	) -> ProgramResult {
		self.sol_log("SyscallStubs: sol_invoke_signed() not available");
		Ok(())
	}
	fn sol_get_clock_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_epoch_schedule_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_fees_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_rent_sysvar(&self, _var_addr: *mut u8) -> u64 {
		UNSUPPORTED_SYSVAR
	}
	fn sol_get_return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
		None
	}
	fn sol_set_return_data(&self, _data: &[u8]) {}
	fn sol_log_data(&self, fields: &[&[u8]]) {
		print!("data:");
		for field in fields.iter() {
			print!(" {}", base64::encode(field));
		}
		println!("");
	}
	fn sol_get_processed_sibling_instruction(&self, _index: usize) -> Option<Instruction> {
		None
	}
	fn sol_get_stack_height(&self) -> u64 {
		0
	}
}

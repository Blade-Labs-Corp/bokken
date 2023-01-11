use std::slice::Iter;

use blade_labs_sol_program_common::serialization::Castable;
use solana_program::{account_info::{AccountInfo, next_account_info}, pubkey::Pubkey, program_error::ProgramError, msg};
use std::backtrace::Backtrace;

use crate::state::TestProgramState;


fn test_increment_func(
	test_state: &mut TestProgramState,
	num: u64
) -> Result<(), ProgramError> {
	test_state.property1 += num;
	test_state.property2 += num * 2;
	msg!("Look ma, a stacktrace! {}", Backtrace::force_capture());
	Ok(())
}

pub fn process_increment_number(
	program_id: &Pubkey,
	account_iter: &mut Iter<AccountInfo>,
	number: u64
) -> Result<(), ProgramError> {
	msg!("Program ID: {}", program_id);
	msg!("number: {}", number);
	let mut test_state = TestProgramState::from_account_info_mut(
		next_account_info(account_iter)?
	)?;
	msg!("Old test_state: {:#?}", test_state);
	test_increment_func(&mut test_state, number)?;
	msg!("Old new_state: {:#?}", test_state);
	Ok(())
}

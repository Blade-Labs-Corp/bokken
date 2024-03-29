use std::{slice::Iter, cell::RefMut};

use solana_program::{account_info::{AccountInfo, next_account_info}, pubkey::Pubkey, program_error::ProgramError, msg, program::invoke, instruction::{Instruction, AccountMeta}, clock::Clock, sysvar::Sysvar};
use std::backtrace::Backtrace;

use crate::{state::TestProgramState, instruction::TestProgramInstruction};


fn test_increment_func(
	test_state: &mut TestProgramState,
	num: u64
) -> Result<(), ProgramError> {
	test_state.property1 += num;
	test_state.property2 += num * 2;
	msg!("Look ma, a stacktrace!\n{}", Backtrace::force_capture());
	Ok(())
}

pub fn process_increment_number(
	program_id: &Pubkey,
	account_iter: &mut Iter<AccountInfo>,
	number: u64
) -> Result<(), ProgramError> {
	let clock = Clock::get()?;
	msg!("Program ID: {}", program_id);
	msg!("Unix timestamp: {}", clock.unix_timestamp);
	msg!("number: {}", number);
	let test_state_account = next_account_info(account_iter)?;
	let mut test_state = RefMut::map(test_state_account.data.borrow_mut(), |bytes| {
		bytemuck::from_bytes_mut(bytes)
	});
	msg!("Old test_state: {:#?}", test_state);
	test_increment_func(&mut test_state, number)?;
	msg!("New test_state: {:#?}", test_state);
	Ok(())
}

pub fn process_recurse_then_increment_number (
	program_id: &Pubkey,
	account_iter: &mut Iter<AccountInfo>,
	depth: u8,
	number: u64
) -> Result<(), ProgramError> {
	let test_state = next_account_info(account_iter)?;
	msg!("cur depth: {}", depth);
	invoke(
		&Instruction::new_with_borsh(
			*program_id,
			&if depth == 0 {
				TestProgramInstruction::IncrementNumber {
					amount: number
				}
			}else{
				TestProgramInstruction::RecurseThenIncrementNumber {
					call_depth: depth - 1,
					amount: number
				}
			},
			vec![
				AccountMeta {
					pubkey: test_state.key.clone(),
					is_signer: false,
					is_writable: true
				}
			]
		),
		&[test_state.clone()]
	)?;
	Ok(())
}

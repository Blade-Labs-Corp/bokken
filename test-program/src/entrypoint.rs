use blade_labs_sol_program_common::blade_entrypoint;
use solana_program::{
	account_info::AccountInfo,
	pubkey::Pubkey,
	msg, program_error::ProgramError,
};

use crate::{instruction::TestProgramInstruction, processor::{process_increment_number, process_recurse_then_increment_number}};

blade_entrypoint!(process_instruction);
fn process_instruction<'a>(
	program_id: &'a Pubkey,
	accounts: &'a [AccountInfo<'a>],
	instruction_data: &'a [u8],
) -> Result<(), ProgramError> {
	let mut account_info_iter = accounts.iter();
	let instruction = TestProgramInstruction::unpack(instruction_data)?;
	match instruction {
		TestProgramInstruction::HelloWorld => {
			msg!("ix: HelloWorld");
			// that's it
		}
		TestProgramInstruction::IncrementNumber { amount } => {
			msg!("ix: IncrementNumber");
			process_increment_number(program_id, &mut account_info_iter, amount)?;
		},
		TestProgramInstruction::RecurseThenIncrementNumber { call_depth, amount } => {
			msg!("ix: RecurseThenIncrementNumber");
			process_recurse_then_increment_number(
				program_id,
				&mut account_info_iter,
				call_depth,
				amount
			)?;
		}
	}
	Ok(())
}

use borsh::BorshDeserialize;
use solana_program::{msg, program_error::ProgramError, pubkey::Pubkey};

#[derive(BorshDeserialize, PartialEq, Eq, Debug)]
/// ts-autogen: program-instruction
pub enum TestProgramInstruction {
	HelloWorld,
	/// Accounts expected:
	///
	/// 0. `[writable]` test_account: This program ID,
	IncrementNumber {
		amount: u64
	},
}

impl TestProgramInstruction {
	pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
		TestProgramInstruction::try_from_slice(input).map_err(|err| {
			msg!("Couldn't parse instruction: {}", err.to_string());
			ProgramError::InvalidInstructionData
		})
	}
}

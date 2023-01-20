use bytemuck::{Zeroable, Pod};

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
/// ts-autogen
pub struct TestProgramState {
	pub property1: u64,
	pub property2: u64
}

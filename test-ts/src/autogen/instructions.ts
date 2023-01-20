import {
	AccountMeta,
	PublicKey,
	TransactionInstruction,
	SystemProgram,
	SYSVAR_RENT_PUBKEY,
	SYSVAR_CLOCK_PUBKEY,
	SYSVAR_STAKE_HISTORY_PUBKEY,
	STAKE_CONFIG_ID,
	StakeProgram
} from "@solana/web3.js";

export const TOKEN_PROGRAM_PUBKEY = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
export const ATOKEN_PROGRAM_PUBKEY = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
import { encode, } from "./serialization"
export class TestProgramInstructionBuilder {
	static buildHelloWorldIx(
		programId: PublicKey,
	): TransactionInstruction {
		const [programIxData] = encode.TestProgramInstruction("HelloWorld");
		return new TransactionInstruction({
			programId,
			keys: ([
			]),
			data: programIxData
		});
	};
	static buildIncrementNumberIx(
		programId: PublicKey,
		testAccount: PublicKey,
		amount: bigint,
	): TransactionInstruction {
		const [programIxData] = encode.TestProgramInstruction({
			_enum: "IncrementNumber", amount
		});
		return new TransactionInstruction({
			programId,
			keys: ([
				{
					pubkey: testAccount,
					isSigner: false,
					isWritable: true
				},
			]),
			data: programIxData
		});
	};
	static buildRecurseThenIncrementNumberIx(
		programId: PublicKey,
		testAccount: PublicKey,
		callDepth: number,
		amount: bigint,
	): TransactionInstruction {
		const [programIxData] = encode.TestProgramInstruction({
			_enum: "RecurseThenIncrementNumber", callDepth, amount
		});
		return new TransactionInstruction({
			programId,
			keys: ([
				{
					pubkey: testAccount,
					isSigner: false,
					isWritable: true
				},
			]),
			data: programIxData
		});
	};
};

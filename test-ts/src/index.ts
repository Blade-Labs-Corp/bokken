import {inspect} from "util";
import {Connection, PublicKey, Keypair, Transaction, SystemInstruction, SystemProgram, sendAndConfirmTransaction} from "@solana/web3.js";
import { sizeOf } from "./autogen/serialization";
import {TestProgramInstructionBuilder} from "./autogen/instructions";

(async () => {
	try {
		const connection = new Connection("http://127.0.0.1:8899");
		const programId = new PublicKey("TheDebugab1eProgram111111111111111111111111");
		const testKeypair = Keypair.fromSeed(Buffer.alloc(32, 42));
		console.log("Hello world");

		const stateAccountSeed = "test123";

		const programState = await PublicKey.createWithSeed(testKeypair.publicKey, stateAccountSeed, programId);

		
		console.log("Hello world");
		console.log(
			inspect(
				await sendAndConfirmTransaction(
					connection,
					new Transaction().add(
						SystemProgram.createAccountWithSeed({
							basePubkey: testKeypair.publicKey,
							fromPubkey: testKeypair.publicKey,
							lamports: await connection.getMinimumBalanceForRentExemption(16),
							newAccountPubkey: programState,
							programId,
							seed: stateAccountSeed,
							space: 16
						})
					),
					[testKeypair]
				),
				false,
				Infinity,
				true
			)
		);
		console.log(
			inspect(
				await sendAndConfirmTransaction(
					connection,
					new Transaction().add(
						TestProgramInstructionBuilder.buildIncrementNumberIx(programId, programState, 1337n)
					),
					[testKeypair]
				),
				false,
				Infinity,
				true
			)
		)
		/*
		const testKeypair = Keypair.fromSeed(Buffer.alloc(32, 42));
		console.log("Hello world");
		console.log(
			inspect(
				await connection.simulateTransaction(
					new Transaction().add(
						TestProgramInstructionBuilder.buildHelloWorldIx(programId)
					),
					[testKeypair]
				),
				false,
				Infinity,
				true
			)
		)
		console.log("Increment number");
		console.log(
			inspect(
				await connection.simulateTransaction(
					new Transaction().add(
						TestProgramInstructionBuilder.buildIncrementNumberIx(programId, programState, 1337n)
					),
					[testKeypair]
				),
				false,
				Infinity,
				true
			)
		);
		*/
	}catch(ex: any) {
		if (Array.isArray(ex.logs)) {
			console.error(ex.name, ex.message);
			console.error(ex.logs.join("\n"));
		}else{
			console.error(ex);
		}
		console.error("----------");
		inspect(ex, false, Infinity, true);
		console.error("----------");
		process.exitCode = 1;
	}
})();

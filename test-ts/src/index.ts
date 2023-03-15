import {inspect} from "util";
import {Connection, PublicKey, Keypair, Transaction, SystemProgram, TransactionInstruction, sendAndConfirmTransaction} from "@solana/web3.js";
import { sizeOf, decode } from "./autogen/serialization";
import {TestProgramInstructionBuilder} from "./autogen/instructions";

async function printSimulatedIxThenSend(connection: Connection, ix: TransactionInstruction, keypairs: [Keypair]) {
	console.log(
		inspect(
			await connection.simulateTransaction(
				new Transaction().add(ix),
				keypairs
			),
			false,
			Infinity,
			true
		)
	);
	console.log(
		"sendTx:",
		await connection.sendTransaction(
			new Transaction().add(ix),
			keypairs
		)
	);
}
async function printAccountInfo(connection: Connection, account: PublicKey ) {
	console.log("--", account.toBase58(), "info --")
	const accountInfo = await connection.getAccountInfo(account);
	console.log(
		inspect(
			accountInfo,
			false,
			Infinity,
			true
		)
	)
}
async function printStateAccountInfo(connection: Connection, programState: PublicKey ) {
	console.log("--", programState.toBase58(), "info --")
	const accountInfo = await connection.getAccountInfo(programState);
	console.log(
		inspect(
			accountInfo,
			false,
			Infinity,
			true
		)
	)
	if (accountInfo != null) {
		console.log(
			inspect(
				decode.TestProgramState(accountInfo.data)[0],
				false,
				Infinity,
				true
			)
		)
	}
}

(async () => {
	try {
		const connection = new Connection("http://127.0.0.1:8899");
		const programId = new PublicKey("TheDebugab1eProgram111111111111111111111111");
		const testKeypair = Keypair.fromSeed(Buffer.alloc(32, 42));

		const stateAccountSeed = "test123";

		const programState = await PublicKey.createWithSeed(testKeypair.publicKey, stateAccountSeed, programId);

		
		console.log("-- Hello world ix --");
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
		);
		console.log("-- create state account --");
		await printSimulatedIxThenSend(
			connection,
			SystemProgram.createAccountWithSeed({
				basePubkey: testKeypair.publicKey,
				fromPubkey: testKeypair.publicKey,
				lamports: await connection.getMinimumBalanceForRentExemption(sizeOf.TestProgramState),
				newAccountPubkey: programState,
				programId,
				seed: stateAccountSeed,
				space: sizeOf.TestProgramState
			}),
			[testKeypair]
		);
		await printAccountInfo(connection, testKeypair.publicKey);
		await printStateAccountInfo(connection, programState);
		//*/
		console.log("-- inc number --");
		await printSimulatedIxThenSend(
			connection,
			TestProgramInstructionBuilder.buildIncrementNumberIx(programId, programState, 1337n),
			[testKeypair]
		);
		await printAccountInfo(connection, testKeypair.publicKey);
		await printStateAccountInfo(connection, programState);
		console.log(
			"s&c tx:",
			await sendAndConfirmTransaction(
				connection,
				new Transaction().add(TestProgramInstructionBuilder.buildRecurseThenIncrementNumberIx(
					programId,
					programState,
					2,
					500n
				)),
				[testKeypair]
			)
		);
		await printAccountInfo(connection, testKeypair.publicKey);
		await printStateAccountInfo(connection, programState);
		/*
		console.log("-- inc number again, in a loop --");
		
		while(true){
			console.log(
				"s&c tx:",
				await sendAndConfirmTransaction(
					connection,
					new Transaction().add(TestProgramInstructionBuilder.buildIncrementNumberIx(programId, programState, 9001n)),
					[testKeypair]
				)
			);
			await printStateAccountInfo(connection, programState);
		}
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

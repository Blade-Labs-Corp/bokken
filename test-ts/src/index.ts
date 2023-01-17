import {inspect} from "util";
import {Connection, PublicKey, Keypair, Transaction} from "@solana/web3.js";
import {TestProgramInstructionBuilder} from "./autogen/instructions";

(async () => {
	try {
		const connection = new Connection("http://127.0.0.1:8899");
		const programId = new PublicKey("TheDebugab1eProgram111111111111111111111111");
		const programState = new PublicKey("TheDebugab1eProgramTestState111111111111111");

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

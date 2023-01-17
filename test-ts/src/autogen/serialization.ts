// Auto-generated borsh-ts parser
import { PublicKey } from "@solana/web3.js";
export class FixedPointVaule {
	rawValue: bigint;
	private _divisor: bigint;
	constructor(rawValue: bigint, divisor: bigint) {
		this.rawValue = rawValue;
		this._divisor = divisor;
	}
	get displayValue(): number {
		return Number(this.rawValue) / Number(this.divisor);
	}
	get divisor(): bigint {
		return this._divisor;
	}
	set divisor(newDivisor: bigint) {
		const oldDivisor = this._divisor;
		this.rawValue = this.rawValue * oldDivisor / newDivisor;
		this._divisor = newDivisor;
	}
	correctRawValue(newDivisor: bigint): bigint {
		if (newDivisor == this.divisor) {
			return this.rawValue;
		} else {
			return this.rawValue * newDivisor / this.divisor;
		}
	}
	private correctRawValuesForMath(other: FixedPointVaule): [bigint, bigint, bigint] {
		if (this.divisor == other.divisor) {
			return [this.rawValue, other.rawValue, this.divisor];
		} else if (this.divisor < other.divisor) {
			return [this.rawValue * this.divisor / other.divisor, other.rawValue, other.divisor];
		} else {
			return [this.rawValue, other.rawValue * other.divisor / this.divisor, this.divisor];
		}
	}
	add(value: FixedPointVaule): FixedPointVaule {
		const [thisValue, otherValue, divisor] = this.correctRawValuesForMath(value);
		return new FixedPointVaule(
			thisValue + otherValue,
			divisor
		);
	}
	sub(value: FixedPointVaule): FixedPointVaule {
		const [thisValue, otherValue, divisor] = this.correctRawValuesForMath(value);
		return new FixedPointVaule(
			thisValue + otherValue,
			divisor
		);
	}
	mul(value: FixedPointVaule): FixedPointVaule {
		const [thisValue, otherValue, divisor] = this.correctRawValuesForMath(value);
		return new FixedPointVaule(
			thisValue * otherValue / divisor,
			divisor
		);
	}
	div(value: FixedPointVaule): FixedPointVaule {
		const [thisValue, otherValue, divisor] = this.correctRawValuesForMath(value);
		return new FixedPointVaule(
			thisValue * divisor / otherValue,
			divisor
		);
	}
}
export class FreshValue<T> {
	private rawValue: T;
	readonly timestamp: number;
	readonly slot: number;
	constructor(rawValue: T, timestamp: number | bigint, slot: number | bigint) {
		this.rawValue = rawValue;
		this.timestamp = typeof timestamp == "bigint" ? Number(timestamp) : timestamp;
		this.slot = typeof slot == "bigint" ? Number(slot) : slot;
	}
	getFreshValue(msTolerance: number = 0, curTime: number = Date.now()): T | null {
		if (this.timestamp == 0 || this.slot == 0) {
			return null;
		}
		if ((curTime - this.timestamp) <= msTolerance) {
			return this.rawValue;
		}
		return null;
	}
}
type TestProgramInstruction_HelloWorld = "HelloWorld";
type TestProgramInstruction_IncrementNumber = {
	_enum: "IncrementNumber"
	amount: bigint;
};
export type TestProgramInstruction = TestProgramInstruction_HelloWorld | TestProgramInstruction_IncrementNumber;

export namespace encode {
	export function TestProgramInstruction(obj: TestProgramInstruction, curBuf: (Buffer | undefined) = Buffer.allocUnsafe(1), i: (number | undefined) = 0): [Buffer, number] {
		let bufs: Buffer[] = []; let totalLen = 0;
		switch (typeof obj == "string" ? obj : (obj as any)._enum) {
			case "HelloWorld":
				curBuf[i++] = 0;
				break;
			case "IncrementNumber":
				curBuf[i++] = 1;
				bufs.push(curBuf);
				totalLen += curBuf.length;
				curBuf = Buffer.allocUnsafe(8); i = 0;
				curBuf.writeBigUInt64LE((obj as any).amount, i);
				i += 8;
				break;
			default:
				throw new Error("Unknown enum type");
		}
		if (!bufs.length) {
			return [curBuf!, i!];
		}
		if (curBuf != null) {
			bufs.push(curBuf);
			totalLen += curBuf.length;
		}
		return [Buffer.concat(bufs, totalLen), 0];
	}

};
export namespace decode {
	export function TestProgramInstruction(buf: Buffer): [TestProgramInstruction, Buffer] {
		let result: any; let i = 0;
		switch (buf[i++]) {
			case 0:
				result = "HelloWorld";
				break;
			case 1:
				result = {};
				result._enum = "IncrementNumber";
				result.amount = (() => {
					const subResult = buf.readBigUInt64LE(i);
					i += 8;
					return subResult;
				})();
				break;
			default:
				throw new Error("Unknown enum type");
		}
		return [result, buf.subarray(i)];
	}

};
export namespace sizeOf {
	export namespace TestProgramInstruction {
		export const HelloWorld = 1;
		export const IncrementNumber = 9;
	};

};

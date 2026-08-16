//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Provides pure helpers for the Python build-system benchmark contract.

import { chmodSync, readFileSync, rmSync } from "node:fs";

export type Runner = "bazel" | "bsmr";

export interface BsmrOutputs {
	source: string;
	wheel: string;
}

export interface WheelEntry {
	crc32: number;
	name: string;
	size: number;
}

export interface PerformanceGateResult {
	maximumBsmrToBazelRatio: number;
	observedBsmrToBazelRatio: number;
	pass: boolean;
	regime: string;
}

const performanceBudgets = {
	"acquisition-cold": 0.8,
	"leaf-runtime": 0.75,
	"leaf-test": 1.1,
	"leaf-wheel": 1.2,
	"output-restoration": 0.3,
	"provisioned-cold": 0.8,
	"resident-noop": 0.3,
	"shared-cache-fresh-checkout": 0.3,
	"test-cached": 0.4,
	"test-first": 0.5,
} as const;

export const targetSource = "root//:__bsmr_python_sources";
export const targetWheel = "root//:django";

/** Parses one bounded positive integer environment setting. */
export function positiveInteger(name: string, defaultValue: number, minimum: number): number {
	const value = Number.parseInt(process.env[name] ?? String(defaultValue), 10);
	if (!Number.isSafeInteger(value) || value < minimum) throw new Error(`${name} must be an integer of at least ${minimum}`);
	return value;
}

/** Returns alternating runner order to cancel first-run thermal and cache bias. */
export function runnerOrder(iteration: number): readonly Runner[] {
	return iteration % 2 === 1 ? ["bsmr", "bazel"] : ["bazel", "bsmr"];
}

/** Returns the middle sample after a numeric sort. */
export function median(values: readonly number[]): number {
	if (values.length === 0) throw new Error("cannot compute a median without samples");
	const sorted = [...values].sort((left, right) => left - right);
	return sorted[Math.floor(sorted.length / 2)]!;
}

/** Removes a read-only directory tree such as a Bazel tree artifact. */
export function removeReadOnlyTree(path: string): void {
	chmodSync(path, 0o755);
	rmSync(path, { recursive: true });
}

/** Returns payload paths whose presence, size, or content differs. */
export function changedWheelEntries(left: readonly WheelEntry[], right: readonly WheelEntry[]): readonly string[] {
	const leftByName = new Map(left.map((entry) => [entry.name, entry]));
	const rightByName = new Map(right.map((entry) => [entry.name, entry]));
	return [...new Set([...leftByName.keys(), ...rightByName.keys()])]
		.filter((name) => JSON.stringify(leftByName.get(name)) !== JSON.stringify(rightByName.get(name)))
		.sort();
}

/** Evaluates the semantics-matched release budgets against paired medians. */
export function performanceGateResults(medians: Readonly<Record<string, number>>): readonly PerformanceGateResult[] {
	return Object.entries(performanceBudgets).map(([regime, maximumBsmrToBazelRatio]) => {
		const bsmr = medians[`${regime}:bsmr`];
		const bazel = medians[`${regime}:bazel`];
		if (bsmr === undefined || bazel === undefined || bsmr <= 0 || bazel <= 0) throw new Error(`missing positive paired medians for ${regime}`);
		const observedBsmrToBazelRatio = bsmr / bazel;
		return { maximumBsmrToBazelRatio, observedBsmrToBazelRatio, pass: observedBsmrToBazelRatio <= maximumBsmrToBazelRatio, regime };
	});
}

/** Parses the typed BSMR outputs needed for correctness and restoration gates. */
export function parseBsmrOutputs(stdout: string): BsmrOutputs {
	const value: unknown = JSON.parse(stdout.trim());
	if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("BSMR output must be a JSON object");
	const outputs = value as Record<string, unknown>;
	if (typeof outputs[targetWheel] !== "string" || typeof outputs[targetSource] !== "string") throw new Error("BSMR omitted a requested Python output");
	return { source: outputs[targetSource], wheel: outputs[targetWheel] };
}

/** Reads the central directory without trusting an ambient archive tool. */
export function wheelPayload(path: string, prefix: string): readonly WheelEntry[] {
	const archive = readFileSync(path);
	const minimum = Math.max(0, archive.length - 65_557);
	let end = -1;
	for (let offset = archive.length - 22; offset >= minimum; offset -= 1) {
		if (archive.readUInt32LE(offset) === 0x06054b50) {
			end = offset;
			break;
		}
	}
	if (end < 0) throw new Error(`wheel has no ZIP central directory: ${path}`);
	const count = archive.readUInt16LE(end + 10);
	let offset = archive.readUInt32LE(end + 16);
	if (count === 0xffff || offset === 0xffffffff) throw new Error(`ZIP64 wheel payloads are unsupported: ${path}`);
	const entries: WheelEntry[] = [];
	for (let index = 0; index < count; index += 1) {
		if (archive.readUInt32LE(offset) !== 0x02014b50) throw new Error(`wheel has an invalid central-directory entry: ${path}`);
		const nameLength = archive.readUInt16LE(offset + 28);
		const extraLength = archive.readUInt16LE(offset + 30);
		const commentLength = archive.readUInt16LE(offset + 32);
		const name = archive.subarray(offset + 46, offset + 46 + nameLength).toString("utf8");
		if (name.startsWith(prefix)) entries.push({ crc32: archive.readUInt32LE(offset + 16), name, size: archive.readUInt32LE(offset + 24) });
		offset += 46 + nameLength + extraLength + commentLength;
	}
	return entries.sort((left, right) => left.name.localeCompare(right.name));
}

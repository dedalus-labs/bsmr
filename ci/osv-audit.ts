//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the generated Rust dependency-audit action.

import {
	action,
	pathInput,
	type ActionInputValues,
	type ScriptExec,
} from "@dedalus-labs/hollywood/action-runtime";

const inputs = {
	scanner: pathInput({ description: "Pinned OSV Scanner executable." }),
	report: pathInput({ description: "Path for the scanner JSON report." }),
} as const;

type Inputs = ActionInputValues<typeof inputs>;
type JsonObject = Record<string, unknown>;
type ReadText = (path: string) => Promise<string>;
type Vulnerability = Readonly<{ id: string; summary: string }>;

/** Require one JSON object at a named report path. */
function object(value: unknown, path: string): JsonObject {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		throw new Error(`${path} must be an object`);
	}
	return value as JsonObject;
}

/** Require one JSON array field. */
function arrayField(parent: JsonObject, field: string, path: string): readonly unknown[] {
	const value = parent[field];
	if (!Array.isArray(value)) throw new Error(`${path}.${field} must be an array`);
	return value;
}

/** Require one JSON string field. */
function stringField(parent: JsonObject, field: string, path: string): string {
	const value = parent[field];
	if (typeof value !== "string") throw new Error(`${path}.${field} must be a string`);
	return value;
}

/** Return whether every affected range is explicitly informational-only. */
function isUnmaintainedOnly(vulnerability: JsonObject, path: string): boolean {
	const affected = arrayField(vulnerability, "affected", path);
	if (affected.length === 0) throw new Error(`${path}.affected must not be empty`);
	return affected.every((value, index) => {
		const entry = object(value, `${path}.affected[${index}]`);
		if (entry["database_specific"] === undefined) return false;
		const database = object(entry["database_specific"], `${path}.affected[${index}].database_specific`);
		return database["informational"] === "unmaintained";
	});
}

/** Extract every vulnerability that is not explicitly unmaintained-only. */
export function actionableVulnerabilities(report: unknown): readonly Vulnerability[] {
	const root = object(report, "report");
	const findings: Vulnerability[] = [];
	for (const [resultIndex, resultValue] of arrayField(root, "results", "report").entries()) {
		const result = object(resultValue, `report.results[${resultIndex}]`);
		for (const [packageIndex, packageValue] of arrayField(result, "packages", `report.results[${resultIndex}]`).entries()) {
			const pkg = object(packageValue, `report.results[${resultIndex}].packages[${packageIndex}]`);
			for (const [vulnerabilityIndex, vulnerabilityValue] of arrayField(pkg, "vulnerabilities", `report.results[${resultIndex}].packages[${packageIndex}]`).entries()) {
				const path = `report.results[${resultIndex}].packages[${packageIndex}].vulnerabilities[${vulnerabilityIndex}]`;
				const vulnerability = object(vulnerabilityValue, path);
				if (!isUnmaintainedOnly(vulnerability, path)) {
					findings.push({
						id: stringField(vulnerability, "id", path),
						summary: stringField(vulnerability, "summary", path),
					});
				}
			}
		}
	}
	return findings;
}

/** Run OSV Scanner and reject operational errors or actionable findings. */
export async function auditRustDependencies(
	exec: ScriptExec,
	readText: ReadText,
	input: Inputs,
): Promise<void> {
	const result = await exec(
		input.scanner,
		[
			"scan",
			"source",
			"--lockfile",
			"Cargo.lock",
			"--lockfile",
			"tools/build/third-party/rust/Cargo.lock",
			"--no-resolve",
			"--format",
			"json",
			"--output-file",
			input.report,
			".",
		],
		{ exitPolicy: "any" },
	);
	if (result.exitCode !== 0 && result.exitCode !== 1) {
		throw new Error(`OSV Scanner exited ${result.exitCode}: ${result.stderr}`);
	}
	const findings = actionableVulnerabilities(JSON.parse(await readText(input.report)));
	if (findings.length !== 0) {
		throw new Error(`actionable OSV vulnerabilities: ${findings.map(({ id, summary }) => `${id}: ${summary}`).join(", ")}`);
	}
}

export const osvAuditAction = action({
	name: "Audit Rust dependencies",
	description: "Reject actionable Rust vulnerabilities without shell control flow.",
	localActionPath: "ci/osv-audit",
	inputs,
	outputs: {},
	run: async ({ exec, fs, input, log }) => {
		await auditRustDependencies(exec, fs.readText, input);
		log.info("No actionable Rust vulnerabilities found");
		return {};
	},
});

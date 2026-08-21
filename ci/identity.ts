//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Rejects upstream product names outside legally required provenance records.

import { realpathSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { nodeExec, type ScriptExec } from "@dedalus-labs/hollywood";

const upstreamProduct = ["bu", "ck"].join("");
const prohibited = new RegExp(upstreamProduct, "ig");
const legalFiles = new Set(["NOTICE", "UPSTREAM_CHANGELOG.md"]);

export type TrackedText = Readonly<{ path: string; text: string }>;

/** Return whether the occurrence belongs to an unrelated word such as bucket. */
function isOrdinaryWord(line: string, index: number): boolean {
	return line.slice(index).toLowerCase().startsWith(`${upstreamProduct}et`);
}

/** Return whether a line is the immutable source-attribution marker. */
function isProvenance(line: string): boolean {
	return (
		line.includes(`Upstream-Source: facebook/${upstreamProduct}2@`) ||
		line.includes(`https://github.com/facebook/${upstreamProduct}2`) ||
		line.includes(`https://${upstreamProduct}2.build`)
	);
}

/** Return whether a value contains a prohibited product token. */
function containsProhibited(value: string): boolean {
	return Array.from(value.matchAll(prohibited)).some(
		(match) => match.index !== undefined && !isOrdinaryWord(value, match.index),
	);
}

/** Find prohibited upstream product references in tracked paths and text. */
export function identityFindings(entries: readonly TrackedText[]): readonly string[] {
	const findings: string[] = [];
	for (const entry of entries) {
		if (containsProhibited(entry.path)) {
			findings.push(`${entry.path}: upstream product name in path`);
		}
		if (legalFiles.has(entry.path) || entry.text.includes("\0")) continue;
		for (const [lineIndex, line] of entry.text.split("\n").entries()) {
			if (line.length > 4096) continue;
			for (const match of line.matchAll(prohibited)) {
				if (match.index === undefined || isOrdinaryWord(line, match.index) || isProvenance(line)) {
					continue;
				}
				findings.push(`${entry.path}:${lineIndex + 1}: upstream product name`);
			}
		}
	}
	return findings;
}

/** Read every tracked text file through one bounded Git inventory. */
async function trackedText(root: string, exec: ScriptExec): Promise<readonly TrackedText[]> {
	const inventory = await exec("git", ["ls-files", "-z"], { cwd: root });
	return inventory.stdout
		.split("\0")
		.filter(Boolean)
		.map((path) => ({ path, text: readFileSync(join(root, path), "utf8") }));
}

/** Enforce Bessemer-owned naming across the repository. */
export async function checkIdentity(root: string, exec: ScriptExec): Promise<void> {
	const findings = identityFindings(await trackedText(root, exec));
	if (findings.length !== 0) {
		throw new Error(`repository identity check failed (${findings.length}):\n${findings.join("\n")}`);
	}
}

async function main(): Promise<void> {
	const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
	await checkIdentity(root, nodeExec);
}

if (process.argv[1] !== undefined && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
	void main();
}

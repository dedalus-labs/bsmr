//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies one downloaded artifact against its pinned SHA-256 digest.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

/** Reject an artifact whose bytes do not match the pinned digest. */
export async function verifySha256(path: string, expected: string): Promise<void> {
	if (!/^[0-9a-f]{64}$/.test(expected)) throw new Error(`invalid SHA-256 digest: ${expected}`);
	const contents = await readFile(path);
	const actual = createHash("sha256").update(contents).digest("hex");
	if (actual !== expected) throw new Error(`${path}: SHA-256 mismatch: got ${actual}, expected ${expected}`);
}

/** Parse the command line and verify exactly one artifact. */
async function main(): Promise<void> {
	const [path, expected, ...extra] = process.argv.slice(2);
	if (path === undefined || expected === undefined || extra.length !== 0) {
		throw new Error("usage: node ci/verify-sha256.ts <path> <sha256>");
	}
	await verifySha256(path, expected);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();

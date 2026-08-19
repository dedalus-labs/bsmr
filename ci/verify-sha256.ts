//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies one downloaded artifact against its pinned SHA-256 digest.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import { action, pathInput, stringInput } from "@dedalus-labs/hollywood/action-runtime";

/** Reject an artifact whose bytes do not match the pinned digest. */
export async function verifySha256(path: string, expected: string): Promise<void> {
	if (!/^[0-9a-f]{64}$/.test(expected)) throw new Error(`invalid SHA-256 digest: ${expected}`);
	const contents = await readFile(path);
	const actual = createHash("sha256").update(contents).digest("hex");
	if (actual !== expected) throw new Error(`${path}: SHA-256 mismatch: got ${actual}, expected ${expected}`);
}
export const verifySha256Action = action({
	name: "Verify SHA-256",
	description: "Reject a downloaded artifact whose digest differs from its pin.",
	localActionPath: "ci/verify-sha256",
	inputs: {
		path: pathInput({ description: "Downloaded artifact path." }),
		expected: stringInput({ description: "Expected lowercase SHA-256 digest." }),
	},
	outputs: {},
	run: async ({ input }) => {
		await verifySha256(input.path, input.expected);
		return {};
	},
});

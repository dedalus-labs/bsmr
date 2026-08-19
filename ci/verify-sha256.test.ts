//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies downloaded artifact checksum enforcement.

import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { verifySha256 } from "./verify-sha256.ts";

test("artifact bytes must match the pinned digest", async () => {
	const directory = await mkdtemp(join(tmpdir(), "bsmr-sha256-"));
	try {
		const path = join(directory, "artifact");
		await writeFile(path, "bessemer\n");
		const digest = "257bd14e6e0e864404def0ccce19ca4d2a10e34154938d2ad18ede6f3250d181";
		await assert.doesNotReject(verifySha256(path, digest));
		await assert.rejects(verifySha256(path, "0".repeat(64)), /SHA-256 mismatch/);
	} finally {
		await rm(directory, { force: true, recursive: true });
	}
});

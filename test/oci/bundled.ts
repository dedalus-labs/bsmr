//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies that a selected engine contains the exact current source prelude.

import assert from "node:assert/strict";
import type { ScriptExec } from "@dedalus-labs/hollywood";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** Match the bundler's regular-file paths, exact contents, and executable bits. */
function snapshot(root: string, prefix = ""): Record<string, { sha256: string; executable: boolean }> {
	const files: Record<string, { sha256: string; executable: boolean }> = {};
	for (const entry of readdirSync(join(root, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, "en"))) {
		const path = prefix ? `${prefix}/${entry.name}` : entry.name;
		if (entry.isDirectory()) Object.assign(files, snapshot(root, path));
		else if (entry.isFile()) files[path] = {
			sha256: createHash("sha256").update(readFileSync(join(root, path))).digest("hex"),
			executable: (statSync(join(root, path)).mode & 0o111) !== 0,
		};
	}
	return files;
}

/** Expand a fresh test workspace's bundled cell, verify it, then remove its inspection copy. */
export async function verifyBundledPrelude(binary: string, source: string, cwd: string,
	execute: ScriptExec, evidence?: string): Promise<string> {
	const expanded = join(cwd, "prelude");
	assert.equal(existsSync(expanded), false, "bundled qualification requires no source-prelude overlay");
	assert.equal(existsSync(join(cwd, ".bsmr.local")), false, "bundled qualification requires the unmodified external-cell configuration");
	try {
		await execute(binary, ["expand-external-cell", "prelude"]);
		const expected = snapshot(source), actual = snapshot(expanded);
		const expectedDigest = createHash("sha256").update(JSON.stringify(expected)).digest("hex");
		const actualDigest = createHash("sha256").update(JSON.stringify(actual)).digest("hex");
		const differingFiles = [...new Set([...Object.keys(expected), ...Object.keys(actual)])]
			.filter((path) => JSON.stringify(expected[path]) !== JSON.stringify(actual[path])).sort();
		if (evidence) writeFileSync(join(evidence, "bundled-prelude.json"), JSON.stringify({
			sourceSha256: expectedDigest, bundledSha256: actualDigest, files: Object.keys(expected).length, differingFiles,
		}, null, 2) + "\n");
		assert.deepEqual(differingFiles, [], "OCI_BUNDLED_PRELUDE_MISMATCH: selected engine differs from the source prelude");
		return expectedDigest;
	} finally {
		rmSync(expanded, { recursive: true, force: true });
	}
}

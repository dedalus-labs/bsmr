//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Checks the committed CLI reference against the built parser.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** Reject any difference between generated and committed CLI documentation. */
export function verifyCliReference(expected: string, actual: string): void {
	if (actual !== expected) throw new Error("docs/reference/cli.md is stale; regenerate it from the built BSMR parser");
}

/** Generate the CLI reference from one built BSMR executable. */
function main(): void {
	const [bsmr, expectedPath, ...extra] = process.argv.slice(2);
	if (bsmr === undefined || expectedPath === undefined || extra.length !== 0) {
		throw new Error("usage: node ci/cli-reference.ts <bsmr> <expected-markdown>");
	}
	const generated = spawnSync(bsmr, ["docs", "markdown-help-doc", "all"], { encoding: "utf8" });
	if (generated.error !== undefined) throw generated.error;
	if (generated.status !== 0) throw new Error(`BSMR CLI generation exited ${String(generated.status)}: ${generated.stderr}`);
	verifyCliReference(readFileSync(expectedPath, "utf8"), generated.stdout);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();

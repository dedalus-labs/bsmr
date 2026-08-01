//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the license policy against an isolated source inventory.

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood";

import { runLicensePolicy } from "./license.ts";

test("policy accepts canonical source and package licenses", async () => {
	const root = mkdtempSync(join(tmpdir(), "bsmr-license-"));
	try {
		mkdirSync(join(root, "app"));
		writeFileSync(join(root, "LICENSE"), "Apache\n");
		writeFileSync(join(root, "LICENSE-APACHE"), "Apache\n");
		writeFileSync(join(root, "app/BUCK"), "# ===----------------------------------------------------------------------===\n# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors\n# SPDX-License-Identifier: Apache-2.0\n# ===----------------------------------------------------------------------===\n\n# Defines build targets for app.\n");
		writeFileSync(join(root, "package.json"), '{"license":"Apache-2.0"}\n');
		const exec: ScriptExec = async (_file, args) => {
			if (args.includes("ls-files")) return { exitCode: 0, stdout: "app/BUCK\0package.json\0", stderr: "" };
			if (args.includes("--name-status")) return { exitCode: 0, stdout: "A\0app/BUCK\0A\0package.json\0", stderr: "" };
			if (args[0] === "metadata") return { exitCode: 0, stdout: '{"packages":[]}', stderr: "" };
			return { exitCode: 0, stdout: "", stderr: "" };
		};
		await assert.doesNotReject(runLicensePolicy("check", root, exec));
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});

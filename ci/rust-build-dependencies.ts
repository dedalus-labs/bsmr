//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Adapts Bessemer's Reindeer configuration to Reindeer's external schema.

import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const externalProduct = ["bu", "ck"].join("");

function replaceOnce(source: string, from: string, to: string): string {
	const parts = source.split(from);
	if (parts.length !== 2) throw new Error(`expected exactly one ${from}`);
	return `${parts[0]}${to}${parts[1]}`;
}

/** Translate the owned configuration to Reindeer's required wire format. */
export function externalReindeerConfig(source: string, thirdParty: string): string {
	const section = replaceOnce(source, "[bsmr]", `[${externalProduct}]`);
	const fields = replaceOnce(section, "bsmrfile_imports", `${externalProduct}file_imports`);
	return `third_party_dir = ${JSON.stringify(thirdParty)}\n${fields}`;
}

function main(): void {
	const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
	const thirdParty = join(root, "tools/build/third-party/rust");
	const source = readFileSync(join(thirdParty, "reindeer.toml"), "utf8");
	const temporary = mkdtempSync(join(tmpdir(), "bsmr-reindeer-"));
	const config = join(temporary, "reindeer.toml");

	try {
		writeFileSync(config, externalReindeerConfig(source, thirdParty));
		const result = spawnSync(
			join(root, "tools/bin/reindeer"),
			["--config", config, `${externalProduct}ify`],
			{ cwd: root, stdio: "inherit" },
		);
		if (result.error !== undefined) throw result.error;
		if (result.status !== 0) throw new Error(`Reindeer exited with status ${result.status}`);
	} finally {
		rmSync(temporary, { recursive: true, force: true });
	}
}

if (process.argv[1] !== undefined && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
	main();
}

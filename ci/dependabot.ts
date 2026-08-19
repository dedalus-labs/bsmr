//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Generates the repository dependency-update policy.

import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { stringify } from "yaml";

const cargoDirectories = ["/", "/tools/build/third-party/rust"] as const;
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = resolve(repositoryRoot, ".github/dependabot.yml");

/** Define one Cargo update root with compatible and security updates isolated. */
function cargoUpdate(directory: (typeof cargoDirectories)[number]) {
	return {
		"package-ecosystem": "cargo",
		directory,
		schedule: {
			interval: "weekly",
			day: "sunday",
			time: "06:00",
			timezone: "America/Los_Angeles",
		},
		// One routine bundle and one independently reviewed breaking update may be open.
		"open-pull-requests-limit": 2,
		// Routine releases soak before review; security updates bypass cooldowns.
		cooldown: {
			"default-days": 7,
			"semver-major-days": 14,
			"semver-minor-days": 7,
			"semver-patch-days": 3,
		},
		groups: {
			routine: {
				"applies-to": "version-updates",
				patterns: ["*"],
				"update-types": ["minor", "patch"],
			},
			security: { "applies-to": "security-updates", patterns: ["*"] },
		},
		assignees: ["windsornguyen"],
		"commit-message": { prefix: "chore(deps)" },
	};
}

/** Return the complete typed dependency-update policy. */
export function dependabotConfig() {
	return {
		version: 2,
		updates: cargoDirectories.map(cargoUpdate),
	};
}

/** Render deterministic GitHub-compatible YAML. */
export function renderDependabotConfig(): string {
	return `# @generated from ci/dependabot.ts. Do not edit by hand.\n\n${stringify(dependabotConfig(), { aliasDuplicateObjects: false, lineWidth: 0, version: "1.1" })}`;
}

/** Generate the policy or reject stale committed output. */
async function main(): Promise<void> {
	const expected = renderDependabotConfig();
	if (process.argv.includes("--check")) {
		const actual = await readFile(outputPath, "utf8");
		if (actual !== expected) throw new Error("dependabot.yml is stale; regenerate it");
		return;
	}
	await writeFile(outputPath, expected);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();

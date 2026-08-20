//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies release branch synchronization.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import type { ScriptExec } from "@dedalus-labs/hollywood/action-runtime";

import { renderPreamble } from "./license-preamble.ts";
import {
	commitReleaseMetadata,
	consumeReleaseOverride,
	releaseSyncAction,
	synchronizeChangelog,
} from "./release-sync.ts";

type Invocation = Readonly<{
	file: string;
	args: readonly string[];
	exitPolicy: string | undefined;
}>;

function harness(diffExitCode: number): { exec: ScriptExec; invocations: Invocation[] } {
	const invocations: Invocation[] = [];
	return {
		invocations,
		exec: async (file, args, options) => {
			invocations.push({ file, args, exitPolicy: options?.exitPolicy });
			return {
				exitCode: file === "git" && args[0] === "diff" ? diffExitCode : 0,
				stdout: "",
				stderr: "",
			};
		},
	};
}

test("unchanged release metadata does not create an empty commit", async () => {
	const state = harness(0);
	assert.equal(await commitReleaseMetadata(state.exec, "release-please--branches--main"), false);
	assert.deepEqual(state.invocations.map(({ file, args }) => [file, ...args.slice(0, 2)]), [
		["git", "check-ref-format", "--branch"],
		["git", "add", "VERSION"],
		["git", "diff", "--cached"],
	]);
	assert.equal(state.invocations[2]?.exitPolicy, "any");
});

test("changed release metadata is committed to the exact release branch", async () => {
	const state = harness(1);
	assert.equal(await commitReleaseMetadata(state.exec, "release-please--branches--main"), true);
	assert.deepEqual(state.invocations.at(-1), {
		file: "git",
		args: ["push", "origin", "HEAD:release-please--branches--main"],
		exitPolicy: undefined,
	});
});

test("release synchronization accepts GitHub's repository root verbatim", () => {
	assert.deepEqual(Object.keys(releaseSyncAction.inputs), ["branch", "workspace"]);
	assert.equal(releaseSyncAction.inputs.workspace?.kind, "string");
});

test("bundled release action starts through its runtime entrypoint", () => {
	const workspace = mkdtempSync(join(tmpdir(), "bsmr-release-action-"));
	try {
		mkdirSync(join(workspace, "app", "bsmr"), { recursive: true });
		writeFileSync(join(workspace, "VERSION"), "0.0.1\n");
		writeFileSync(join(workspace, ".release-please-manifest.json"), '{".":"0.0.1"}\n');
		writeFileSync(join(workspace, "Cargo.lock"), '[[package]]\nname = "bsmr"\nversion = "0.0.1"\n');
		writeFileSync(join(workspace, "dist-workspace.toml"), 'version = "0.0.1"\n');
		writeFileSync(join(workspace, "app", "bsmr", "Cargo.toml"), 'name = "bsmr"\nversion = "0.0.1"\n');
		writeFileSync(join(workspace, "release-please-config.json"), '{}\n');
		writeFileSync(
			join(workspace, "CHANGELOG.md"),
			`${renderPreamble("CHANGELOG.md", "upstream-modified")}# Changelog\n\nNotable changes to Bessemer are recorded here. Release entries are generated\nfrom conventional commits and reviewed before publication.\n`,
		);
		execFileSync("git", ["init", "--quiet"], { cwd: workspace });
		execFileSync("git", ["add", "."], { cwd: workspace });
		execFileSync("git", ["-c", "user.name=test", "-c", "user.email=test@example.com", "commit", "--quiet", "-m", "fixture"], {
			cwd: workspace,
		});

		const root = join(dirname(fileURLToPath(import.meta.url)), "..");
		execFileSync(process.execPath, [join(root, ".github/actions/ci/release-sync/dist/index.js")], {
			cwd: workspace,
			env: {
				PATH: process.env["PATH"],
				INPUT_BRANCH: "release-please--branches--main",
				INPUT_WORKSPACE: workspace,
			},
		});
	} finally {
		rmSync(workspace, { force: true, recursive: true });
	}
});

test("release synchronization consumes the one-shot version", () => {
	const workspace = mkdtempSync(join(tmpdir(), "bsmr-release-override-"));
	try {
		writeFileSync(join(workspace, "VERSION"), "0.0.1\n");
		writeFileSync(
			join(workspace, "release-please-config.json"),
			'{"packages":{".":{"release-as":"0.0.1","release-type":"simple"}}}\n',
		);
		assert.equal(consumeReleaseOverride(workspace), true);
		const config = JSON.parse(readFileSync(join(workspace, "release-please-config.json"), "utf8")) as {
			packages: Record<string, Record<string, unknown>>;
		};
		assert.equal(config.packages["."]?.["release-as"], undefined);
		assert.equal(consumeReleaseOverride(workspace), false);
	} finally {
		rmSync(workspace, { force: true, recursive: true });
	}
});

test("invariant_release_sync_restores_the_changelog_preamble_once", () => {
	const workspace = mkdtempSync(join(tmpdir(), "bsmr-release-changelog-"));
	try {
		const preamble = renderPreamble("CHANGELOG.md", "upstream-modified").trimEnd();
		writeFileSync(
			join(workspace, "CHANGELOG.md"),
			`# Changelog\n\n## 0.0.2\n\nFixed.\n\n${preamble}\n\n## Changelog\n\nNotable changes to Bessemer are recorded here. Release entries are generated\nfrom conventional commits and reviewed before publication.\n`,
		);
		assert.equal(synchronizeChangelog(workspace), true);
		const changelog = readFileSync(join(workspace, "CHANGELOG.md"), "utf8");
		assert.match(changelog, /^<!-- ===-+=== -->\n<!-- Upstream-Source:/);
		assert.equal(changelog.match(/Upstream-Source:/g)?.length, 1);
		assert.equal(changelog.match(/^# Changelog$/gm)?.length, 1);
		assert.match(changelog, /## 0\.0\.2\n\nFixed\./);
		assert.equal(synchronizeChangelog(workspace), false);
	} finally {
		rmSync(workspace, { force: true, recursive: true });
	}
});

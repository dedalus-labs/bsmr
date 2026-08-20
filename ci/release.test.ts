//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the release version and workflow boundary.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import test from "node:test";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

import { command, expr, format, stepOutput } from "@dedalus-labs/hollywood";

import { releasePlease } from "./release-please.ts";
import { synchronizeReleaseVersion } from "./release-version.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path: string): string => readFileSync(join(root, path), "utf8");

type ReleaseConfig = Readonly<{
	"bootstrap-sha": string;
	packages: Readonly<Record<string, Readonly<Record<string, unknown>>>>;
}>;

test("one product version drives release automation", () => {
	const version = read("VERSION").trim();
	assert.match(version, /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/);
	assert.equal((JSON.parse(read(".release-please-manifest.json")) as Record<string, string>)["."], version);
	assert.match(read("dist-workspace.toml"), new RegExp(`^version = "${version}"$`, "m"));
	assert.match(read("app/bsmr/Cargo.toml"), new RegExp(`^version = "${version}"$`, "m"));
	assert.match(
		read("Cargo.lock"),
		new RegExp(`^\\[\\[package\\]\\]\\nname = "bsmr"\\nversion = "${version}"$`, "m"),
	);

	const config = JSON.parse(read("release-please-config.json")) as ReleaseConfig;
	assert.equal(config["bootstrap-sha"], "1560aca2002865cd73d7cafb22c705cfb640b2bc");
	assert.deepEqual(config.packages["."], {
		"release-type": "simple",
		"version-file": "VERSION",
		"versioning-strategy": "always-bump-patch",
		"include-component-in-tag": false,
		"include-v-in-tag": true,
		"include-v-in-release-name": true,
		draft: true,
		"force-tag-creation": true,
		"always-update": true,
	});
});

test("release version synchronization updates every derived carrier", () => {
	const fixture = mkdtempSync(join(tmpdir(), "bsmr-release-version-"));
	try {
		mkdirSync(join(fixture, "app", "bsmr"), { recursive: true });
		writeFileSync(join(fixture, "VERSION"), "0.0.1\n");
		writeFileSync(join(fixture, ".release-please-manifest.json"), '{".":"0.0.1"}\n');
		writeFileSync(join(fixture, "Cargo.lock"), '[[package]]\nname = "bsmr"\nversion = "0.0.0"\n');
		writeFileSync(join(fixture, "dist-workspace.toml"), '[dist]\nversion = "0.0.0"\n');
		writeFileSync(join(fixture, "app", "bsmr", "Cargo.toml"), 'name = "bsmr"\nversion = "0.0.0"\n');

		assert.deepEqual(synchronizeReleaseVersion(fixture), [
			"app/bsmr/Cargo.toml",
			"Cargo.lock",
			"dist-workspace.toml",
		]);
		for (const path of ["app/bsmr/Cargo.toml", "Cargo.lock", "dist-workspace.toml"]) {
			assert.match(readFileSync(join(fixture, path), "utf8"), /version = "0\.0\.1"/);
		}
		assert.deepEqual(synchronizeReleaseVersion(fixture), []);
	} finally {
		rmSync(fixture, { recursive: true, force: true });
	}
});

test("release app is not an accidental crates.io package", () => {
	assert.match(read("app/bsmr/Cargo.toml"), /^publish = false$/m);
});

test("release preparation dispatches checks before publication", () => {
	const prepare = releasePlease.jobs.prepare;
	assert.deepEqual(prepare?.permissions, {
		actions: "write",
		contents: "write",
		issues: "write",
		"pull-requests": "write",
	});
	const steps = prepare?.steps ?? [];
	assert.equal(steps[0]?.uses, "googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7");
	assert.ok(steps[2] !== undefined && "uses" in steps[2]);
	assert.equal(steps[2].uses, "./.github/actions/ci/release-sync");
	assert.match(steps[2].if ?? "", /steps\.release\.outputs\.prs_created == 'true'/);
	assert.ok(steps[3] !== undefined && "run" in steps[3]);
	assert.deepEqual(steps[3].run, command({
		file: "gh",
		args: [
			"workflow",
			"run",
			"ci.yml",
			"--ref",
			expr<string>("fromJSON(steps.release.outputs.pr).headBranchName"),
		],
	}));
	assert.match(steps[4]?.if ?? "", /steps\.release\.outputs\.release_created == 'true'/);
	assert.ok(steps[4] !== undefined && "run" in steps[4]);
	assert.deepEqual(steps[4].run, command({
		file: "gh",
		args: [
			"workflow",
			"run",
			"release.yml",
			"--ref",
			"main",
			"--field",
			format("tag={0}", stepOutput("release", "tag_name")),
		],
	}));
});

test("dist release builds preserve required Rust cfg flags", () => {
	assert.match(read(".github/workflows/release.yml"), /^env:\n  RUSTFLAGS: "--cfg tokio_unstable"$/m);
});

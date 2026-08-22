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

import { ci } from "./ci.ts";
import { releasePlease } from "./release-please.ts";
import { publishRelease } from "./release-publish.ts";
import { releaseState } from "./release-state.ts";
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
		versioning: "always-bump-patch",
		"include-component-in-tag": false,
		"include-v-in-tag": true,
		"include-v-in-release-name": true,
		"draft-pull-request": true,
		"skip-github-release": true,
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

test("invariant_release_path_guards_optional_pull_request_output", () => {
	const prepare = releasePlease.jobs.prepare;
	assert.deepEqual(prepare?.permissions, {
		actions: "write",
		contents: "write",
		issues: "write",
		"pull-requests": "write",
	});
	const steps = prepare?.steps ?? [];
	assert.equal(steps[1]?.uses, "./.github/actions/ci/release-state");
	assert.equal(steps[2]?.uses, "googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7");
	assert.match(steps[2]?.if ?? "", /steps\.state\.outputs\.state == 'published'/);
	assert.ok(steps[4] !== undefined && "uses" in steps[4]);
	assert.equal(steps[4].uses, "./.github/actions/ci/release-sync");
	assert.match(steps[4].if ?? "", /steps\.release\.outputs\.prs_created == 'true'/);
	assert.deepEqual(steps[4].with, {
		branch: expr<string>("fromJSON(steps.release.outputs.pr || '{}').headBranchName"),
		workspace: expr<string>("github.workspace"),
	});
	assert.equal(steps.length, 5);
});

test("release pull requests run required checks when marked ready", () => {
	assert.deepEqual(ci.on.pull_request, {
		types: ["opened", "synchronize", "reopened", "ready_for_review"],
	});
});

test("unpublished versions cannot advance release please", () => {
	assert.deepEqual(releaseState("", "v0.0.3"), "absent");
	assert.deepEqual(releaseState("false\ttrue\n", "v0.0.3"), "published");
	assert.throws(
		() => releaseState("true\tfalse\n", "v0.0.3"),
		/v0\.0\.3 is still a draft/,
	);
	assert.throws(
		() => releaseState("false\tfalse\n", "v0.0.3"),
		/v0\.0\.3 is published but mutable/,
	);
});

test("release publication retries the current product version", () => {
	assert.deepEqual(publishRelease.on.push, {
		branches: ["main"],
		paths: [".release-please-manifest.json"],
	});
	assert.deepEqual(publishRelease.on.workflow_dispatch, {});
	const steps = publishRelease.jobs.publish?.steps ?? [];
	assert.deepEqual(steps[0]?.with, { ref: "main", "persist-credentials": false });
	assert.equal(steps[1]?.uses, "./.github/actions/ci/release-state");
	assert.ok(steps[2] !== undefined && "run" in steps[2]);
	assert.deepEqual(steps[2].run, command({
		file: "gh",
		args: [
			"workflow",
			"run",
			"release.yml",
			"--repo",
			expr<string>("github.repository"),
			"--ref",
			"main",
			"--field",
			format("tag={0}", stepOutput("state", "tag")),
		],
	}));
});

test("dist release builds preserve required Rust cfg flags", () => {
	assert.match(read(".github/workflows/release.yml"), /^env:\n  RUSTFLAGS: "--cfg tokio_unstable"$/m);
});

test("release builders use trusted Blacksmith caches", () => {
	const config = read("dist-workspace.toml");
	const setup = read(".github/release-build-setup.yml");
	const cache = read(".github/actions/ci/release-cache/action.yml");
	const workflow = read(".github/workflows/release.yml");
	for (const runner of [
		"blacksmith-32vcpu-ubuntu-2204",
		"blacksmith-32vcpu-ubuntu-2204-arm",
		"blacksmith-32vcpu-windows-2025",
		"blacksmith-12vcpu-macos-15",
	]) {
		assert.match(config, new RegExp(`runner = "${runner}"|= "${runner}"`));
	}
	assert.match(config, /^host = "aarch64-apple-darwin"$/m);
	assert.match(config, /^github-build-setup = "\.\.\/release-build-setup\.yml"$/m);
	assert.match(setup, /github\.event_name == 'workflow_dispatch' && github\.ref == 'refs\/heads\/main' && inputs\.tag != 'dry-run'/);
	assert.match(setup, /uses: \.\/\.github\/actions\/ci\/release-cache/);
	assert.equal(cache.match(/github\.event_name == 'workflow_dispatch' && github\.ref == 'refs\/heads\/main' && github\.event\.inputs\.tag != 'dry-run'/g)?.length, 4);
	assert.equal(cache.match(/useblacksmith\/stickydisk@13af8883542ca949a717e70fef89d15edbb29d88/g)?.length, 3);
	assert.equal(cache.match(/\$\{\{ github\.repository \}\}/g)?.length, 3);
	assert.match(cache, /Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/);
	assert.match(workflow, /Mount trusted release caches/);
	assert.doesNotMatch(cache, /useblacksmith\/(?:setup-docker-builder|build-push-action)/);
	assert.doesNotMatch(workflow, /Docker images to be cached|setup-docker-builder|build-push-action/);
	assert.match(read(".github/CODEOWNERS"), /^\/app\/bsmr\/ @[A-Za-z0-9_-]+$/m);
});

test("dist release publishes within the BSMR repository", () => {
	const workflow = read(".github/workflows/release.yml");
	const config = read("dist-workspace.toml");
	assert.doesNotMatch(config, /^github-releases-repo\s*=/m);
	assert.match(config, /^create-release = true$/m);
	assert.doesNotMatch(workflow, /GH_RELEASES_TOKEN|external_repo_commit/);
	assert.match(workflow, /gh release create /);
	assert.doesNotMatch(workflow, /gh release edit |gh release upload /);
	assert.match(workflow, /RELEASE_COMMIT: "\$\{\{ github\.sha \}\}"/);
});

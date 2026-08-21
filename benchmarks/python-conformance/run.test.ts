//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies fail-closed parsing of BSMR's Python conformance inputs.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { assertBsmrCacheState, bsmrBuildArguments, copyProjectSources, darwinBuildTarget, environmentRoot, parseBuildOutputs, sha256File } from "./run.ts";

test("invariant_cache_isolation_is_an_explicit_bsmr_input", () => {
	assert.deepEqual(bsmrBuildArguments(["//:demo"], undefined), ["build", "//:demo", "--show-full-json-output", "--console", "none"]);
	assert.deepEqual(bsmrBuildArguments(["//:demo"], "cold-python"), ["--isolation-dir", "cold-python", "build", "//:demo", "--show-full-json-output", "--console", "none"]);
});

test("invariant_empty_cache_claims_require_a_new_isolation", (context) => {
	const repository = mkdtempSync(join(tmpdir(), "bsmr-python-cache-state-"));
	context.after(() => rmSync(repository, { force: true, recursive: true }));
	assert.doesNotThrow(() => assertBsmrCacheState(repository, "new-isolation", "empty-isolation"));
	assert.throws(() => assertBsmrCacheState(repository, undefined, "empty-isolation"), /requires BSMR_BENCH_ISOLATION_DIR/);
	mkdirSync(join(repository, "bsmr-out", "used-isolation"), { recursive: true });
	assert.throws(() => assertBsmrCacheState(repository, "used-isolation", "empty-isolation"), /already exists/);
	assert.doesNotThrow(() => assertBsmrCacheState(repository, "used-isolation", "repository-local-state-preserved"));
});

test("invariant_darwin_wheels_use_bsmrs_canonical_deployment_target", () => {
	assert.deepEqual(darwinBuildTarget("aarch64-apple-darwin"), {
		hostPlatform: "macosx-13.0-arm64",
		machine: "arm64",
	});
	assert.equal(darwinBuildTarget("aarch64-unknown-linux-gnu"), undefined);
});

test("invariant_environment_output_names_are_closed", () => {
	assert.equal(environmentRoot("/tmp/runtime.manifest.json"), "/tmp/runtime.overlay");
	assert.throws(() => environmentRoot("/tmp/runtime.json"), /expected an environment manifest/);
});

test("invariant_build_outputs_include_every_requested_target", () => {
	const outputs = parseBuildOutputs('{"root//:python":"/tmp/python","root//:uv":"/tmp/uv"}', [
		"root//:python",
		"root//:uv",
	]);
	assert.deepEqual(outputs, { "root//:python": "/tmp/python", "root//:uv": "/tmp/uv" });
	assert.throws(() => parseBuildOutputs('{"root//:python":"/tmp/python"}', ["root//:uv"]), /missing output for root\/\/:uv/);
});

test("invariant_uv_builds_an_immutable_copy_of_declared_project_sources", (context) => {
	const root = mkdtempSync(join(tmpdir(), "bsmr-python-conformance-"));
	context.after(() => rmSync(root, { force: true, recursive: true }));
	const repository = join(root, "repository");
	const destination = join(root, "sources");
	for (const directory of [".git/objects", "build", "demo.egg-info", "src/demo"]) mkdirSync(join(repository, directory), { recursive: true });
	for (const [path, contents] of [
		[".git/HEAD", "ref: refs/heads/main\n"],
		[".git/objects/demo", "object"],
		["build/output", "generated"],
		["demo.egg-info/PKG-INFO", "generated"],
		["pyproject.toml", "[project]\nname='demo'\nversion='1'\n"],
		["src/demo/__init__.py", "VALUE = 1\n"],
	] as const) writeFileSync(join(repository, path), contents);

	assert.deepEqual(copyProjectSources(repository, destination, [".", "src/demo"]), [destination, join(destination, "src/demo")]);
	assert.equal(existsSync(join(destination, ".git/HEAD")), true);
	assert.equal(existsSync(join(destination, "src/demo/__init__.py")), true);
	assert.equal(existsSync(join(destination, "build")), false);
	assert.equal(existsSync(join(destination, "demo.egg-info")), false);
	writeFileSync(join(root, "identity"), "abc");
	assert.equal(sha256File(join(root, "identity")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
});

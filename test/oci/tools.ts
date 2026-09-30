//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies canonical OCI helper module resolution and literal context directories without a compiler.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs, promisify } from "node:util";

const { positionals, values } = parseArgs({ allowPositionals: true, options: {
	platform: { type: "string", default: "linux/arm64" },
	"engine-version": { type: "string", default: "0.0.9" },
	"bundled-prelude": { type: "boolean", default: false },
} });
const [binary, img, prelude, bundle] = positionals;
const platform = values.platform;
assert.ok(platform === "linux/arm64" || platform === "linux/amd64", "test platform must be linux/arm64 or linux/amd64");
assert.ok(binary && img && prelude && bundle, "pass BSMR, img, source prelude, and the provisioned builder bundle");
const run = promisify(execFile);
const executable = resolve(binary);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-oci-tools-")));
const env = { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") };
const options = { cwd: root, env, timeout: 120_000, maxBuffer: 16 * 1024 * 1024 };
const evidenceRoot = process.env["BSMR_OCI_TEST_EVIDENCE"];
if (evidenceRoot !== undefined) mkdirSync(resolve(evidenceRoot), { recursive: true });
const evidence = evidenceRoot === undefined ? undefined : mkdtempSync(join(resolve(evidenceRoot), "tools-"));
let initialized = false;

/** Execute the actual canonical helper and record this build's action executors. */
async function layer(phase: string) {
	const report = join(root, `${phase}.json`);
	try {
		await run(executable, ["build", "//:layer", "--build-report", report, "--console", "simple", "-v=1,stderr,full_failed_command"], options);
	} catch (error) {
		if (evidence !== undefined && existsSync(report)) cpSync(report, join(evidence, `${phase}-failed.json`));
		throw error;
	}
	const result = JSON.parse(readFileSync(report, "utf8"));
	assert.equal(result.success, true);
	const metadata = JSON.parse(readFileSync(resolve(root, result.results["root//:layer"].outputs.DEFAULT[0]), "utf8"));
	assert.match(metadata.digest, /^sha256:[a-f0-9]{64}$/);
	assert.match(metadata.diff_id, /^sha256:[a-f0-9]{64}$/);
	const log = await run(executable, ["log", "what-ran", "--trace-id", result.trace_id, "--format", "json"], options);
	const actions = log.stdout.trim() === "" ? [] : log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	if (evidence !== undefined) {
		cpSync(report, join(evidence, `${phase}-build.json`));
		writeFileSync(join(evidence, `${phase}-actions.json`), JSON.stringify(actions, null, 2) + "\n");
	}
	process.stdout.write(`${JSON.stringify({ phase, trace: result.trace_id, actions: actions.length, digest: metadata.digest })}\n`);
	return { actions, metadata };
}

try {
	assert.equal((await run(executable, ["--version"], { env })).stdout.trim(), `bsmr ${values["engine-version"]}`);
	await run(executable, ["init"], options);
	initialized = true;
	if (!values["bundled-prelude"]) {
		cpSync(resolve(prelude), join(root, "prelude"), { recursive: true });
		writeFileSync(join(root, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	cpSync(resolve(img), join(root, "img"));
	cpSync(process.execPath, join(root, "node"));
	cpSync(join(resolve(bundle), "docker"), join(root, "docker"));
	cpSync(join(resolve(bundle), "daemon-contract.json"), join(root, "contract.json"));
	cpSync(resolve(import.meta.dirname, "fixtures/artifact.bzl"), join(root, "defs.bzl"));
	const pin = readFileSync(join(resolve(bundle), "buildkit-image.txt"), "utf8").trim();
	assert.match(pin, /@sha256:[a-f0-9]{64}$/);
	writeFileSync(join(root, "fixture.txt"), "fixture\n");
	mkdirSync(join(root, "context-dir"));
	writeFileSync(join(root, "context-dir/Dockerfile"), "FROM scratch\n");
	writeFileSync(join(root, "BUILD.bsmr"), `load("@prelude//oci:defs.bzl", "oci_layer")
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load("@prelude//oci:buildkit.bzl", "managed_buildkit", "dockerfile_image")
load(":defs.bzl", "artifact")
artifact(name = "imgtool", binary = "img")
artifact(name = "runtime", binary = "node")
oci_toolchain(name = "oci", img = ":imgtool", node = ":runtime")
oci_layer(name = "layer", platform = "${platform}", files = {"/fixture.txt": "fixture.txt"}, toolchain = ":oci")
managed_buildkit(name = "builder", node = ":runtime", docker = "docker", daemon_contract = "contract.json", image = ${JSON.stringify(pin)})
dockerfile_image(name = "physical_context", context = "context-dir", builder = ":builder", platform = "${platform}", toolchain = ":oci")
`);
	const providers = await run(executable, ["audit", "providers", "//:physical_context", "--console", "simple"], options);
	assert.match(providers.stdout + providers.stderr, /OciImageInfo/);
	if (evidence !== undefined) writeFileSync(join(evidence, "physical-context-analysis.log"), providers.stdout + providers.stderr);
	const cold = await layer("cold");
	assert.equal(cold.actions.length, 1);
	assert.equal(cold.actions[0].reproducer.executor, "Local");
	const warm = await layer("warm");
	assert.equal(warm.actions.length, 0);
	assert.deepEqual(warm.metadata, cold.metadata);
	process.stdout.write(`${JSON.stringify({ evidence, bundledPrelude: values["bundled-prelude"], physicalContextAnalysis: true, helperModuleResolution: true })}\n`);
} finally {
	if (initialized) await run(executable, ["kill"], options);
	rmSync(root, { recursive: true });
}

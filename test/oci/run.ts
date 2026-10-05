//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies the real language graph from pinned Debian packages through native image execution.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { machine, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { verifyBundledPrelude } from "./bundled.ts";
import { timedExec } from "./exec.ts";
import artifact from "./fixtures/artifact.bzl";
import offline from "./fixtures/offline.sh";
import buildFile from "./fixtures/run/recipe.bsmr";
import installCommand from "./fixtures/run/install.sh";
import markerCommand from "./fixtures/run/marker.sh";
import verifyCommand from "./fixtures/run/verify.sh";

const { positionals, values } = parseArgs({ allowPositionals: true, options: {
	platform: { type: "string", default: `linux/${process.arch === "x64" ? "amd64" : process.arch}` },
	"engine-version": { type: "string", default: "0.0.9" },
	"bundled-prelude": { type: "boolean", default: false },
} });
const [binary, img, umoci, runc, base, lock, prelude] = positionals;
assert.ok(binary && img && umoci && runc && base && lock && prelude,
	"pass BSMR, img, umoci, runc, complete base layout, package lock, and source prelude");
assert.equal(positionals.length, 7, "run qualification accepts exactly seven artifact paths");
assert.equal(process.platform, "linux");
assert.equal(process.getuid?.(), 0);
assert.ok(["x64", "arm64"].includes(process.arch));
assert.equal(machine(), process.arch === "x64" ? "x86_64" : "aarch64", "native qualification cannot use architecture emulation");
const platform = values.platform;
assert.equal(platform, `linux/${process.arch === "x64" ? "amd64" : process.arch}`);
const execute = timedExec(300), engine = resolve(binary);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-native-graph-")));
const options = { cwd: root, env: { BSMR_LOCAL_CACHE_DIR: join(root, "cache") } };
const evidenceRoot = process.env["BSMR_OCI_TEST_EVIDENCE"];
if (evidenceRoot) mkdirSync(evidenceRoot, { recursive: true });
const evidence = evidenceRoot ? mkdtempSync(join(resolve(evidenceRoot), "run-graph-")) : undefined;
const locked = JSON.parse(readFileSync(lock, "utf8"));
assert.deepEqual(locked.requested, ["ca-certificates", "curl"]);
assert.equal(locked.platform, platform);
let initialized = false;

/** Build the real graph and preserve action receipts before removing the owned checkout. */
async function build(target: string, phase: string) {
	const report = join(root, `${phase}.json`);
	try {
		const completed = await execute(engine, ["build", target, "--build-report", report, "--console", "simple", "-v=1,stderr,full_failed_command"], options);
		if (evidence) writeFileSync(join(evidence, `${phase}-output.txt`), completed.stdout + completed.stderr);
	} catch (error) {
		if (evidence) writeFileSync(join(evidence, `${phase}-error.txt`), String(error));
		throw error;
	}
	const result = JSON.parse(readFileSync(report, "utf8"));
	assert.equal(result.success, true);
	const log = await execute(engine, ["log", "what-ran", "--trace-id", result.trace_id, "--format", "json"], options);
	const actions = log.stdout.trim() === "" ? [] : log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	if (evidence) {
		cpSync(report, join(evidence, `${phase}-build.json`));
		writeFileSync(join(evidence, `${phase}-actions.json`), JSON.stringify(actions, null, 2) + "\n");
	}
	process.stdout.write(`${JSON.stringify({ phase, trace: result.trace_id, actions: actions.length })}\n`);
	const label = /^(\/\/:\w+)(?:\[(\w+)\])?$/u.exec(target);
	assert.ok(label);
	return { actions, output: resolve(root, result.results[`root${label[1]}`].outputs[label[2] ?? "DEFAULT"][0]) };
}

/** Reject stale resolution identities and corrupted downloads at the real rule boundary. */
async function rejectInvalidLocks() {
	const cases = [
		{ name: "base", value: { ...locked, base: `sha256:${"0".repeat(64)}` }, expected: /DEBIAN_BASE_MISMATCH/u },
		{ name: "platform", value: { ...locked, platform: platform === "linux/amd64" ? "linux/arm64" : "linux/amd64" }, expected: /DEBIAN_BASE_MISMATCH/u },
		{ name: "request", value: { ...locked, requested: ["curl"] }, expected: /DEBIAN_REQUEST_MISMATCH/u },
		{ name: "checksum", value: { ...locked, packages: locked.packages.map((entry: { sha256: string }, index: number) =>
			index === 0 ? { ...entry, sha256: `${entry.sha256[0] === "0" ? "1" : "0"}${entry.sha256.slice(1)}` } : entry) }, expected: /digest|checksum|sha256/iu },
	];
	for (const sample of cases) {
		writeFileSync(join(root, "packages.lock.json"), JSON.stringify(sample.value));
		await assert.rejects(execute(engine, ["build", "//:packages", "--console", "simple"], options), (error: Error) => {
			if (evidence) writeFileSync(join(evidence, `rejected-${sample.name}.txt`), String(error));
			return sample.expected.test(String(error));
		});
	}
	writeFileSync(join(root, "packages.lock.json"), JSON.stringify(locked));
}

try {
	assert.equal((await execute(engine, ["--version"], options)).stdout.trim(), `bsmr ${values["engine-version"]}`);
	await execute(engine, ["init"], options);
	initialized = true;
	const bundledDigest = values["bundled-prelude"]
		? await verifyBundledPrelude(engine, resolve(prelude), root, (file, args) => execute(file, args, options), evidence) : null;
	if (!values["bundled-prelude"]) {
		cpSync(resolve(prelude), join(root, "prelude"), { recursive: true });
		writeFileSync(join(root, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	if (evidence) writeFileSync(join(evidence, "runtime.json"), JSON.stringify({
		platform, kernel: release(), machine: machine(), engineVersion: values["engine-version"], bundledPrelude: values["bundled-prelude"], bundledSha256: bundledDigest,
		tools: Object.fromEntries(Object.entries({ bsmr: engine, img, umoci, runc, node: process.execPath })
			.map(([name, path]) => [name, createHash("sha256").update(readFileSync(path)).digest("hex")])),
	}, null, 2) + "\n");
	for (const [name, source] of Object.entries({ img, umoci, runc, node: process.execPath })) cpSync(resolve(source), join(root, name));
	cpSync(resolve(base), join(root, "base"), { recursive: true });
	cpSync(resolve(lock), join(root, "packages.lock.json"));
	writeFileSync(join(root, "artifact.bzl"), artifact);
	writeFileSync(join(root, "offline.sh"), offline);
	writeFileSync(join(root, "marker"), "initial\n");
	writeFileSync(join(root, "BUILD.bsmr"), buildFile);
	const fixture = { platform, install: installCommand, verify: verifyCommand };
	writeFileSync(join(root, "fixture.json"), JSON.stringify(fixture));
	const cold = await build("//:layout", "cold");
	assert.ok(cold.actions.length > 0);
	const config = await build("//:verified[config]", "config");
	const inherited = JSON.parse(readFileSync(config.output, "utf8"));
	assert.deepEqual(inherited.config.Env, ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"]);
	assert.deepEqual(inherited.config.Cmd, ["bash"]);
	const warm = await build("//:layout", "warm");
	assert.equal(warm.actions.length, 0);
	await execute(engine, ["clean"], options);
	const restarted = await build("//:layout", "restart");
	assert.ok(restarted.actions.length > 0);
	assert.ok(restarted.actions.every((action) => action.reproducer.executor === "Cache"), JSON.stringify(restarted.actions));
	writeFileSync(join(root, "marker"), "changed\n");
	const edited = await build("//:layout", "input-change");
	assert.ok(edited.actions.some((action) => action.reproducer.executor === "Local"));
	writeFileSync(join(root, "fixture.json"), JSON.stringify({ ...fixture, install: installCommand + markerCommand }));
	const command = await build("//:layout", "command-change");
	assert.ok(command.actions.some((action) => action.reproducer.executor === "Local"));
	await execute(resolve(umoci), ["unpack", "--image", `${command.output}:run`, join(root, "unpacked")], options);
	assert.equal(readFileSync(join(root, "unpacked/rootfs/image-marker"), "utf8"), "changed\n");
	assert.equal(readFileSync(join(root, "unpacked/rootfs/command-marker"), "utf8"), "command");
	if (evidence) {
		await execute("/bin/tar", ["-cf", join(evidence, "layout.tar"), "-C", command.output, "."], options);
		cpSync(join(root, "BUILD.bsmr"), join(evidence, "BUILD.bsmr"));
		cpSync(join(root, "fixture.json"), join(evidence, "fixture.json"));
		cpSync(join(root, "packages.lock.json"), join(evidence, "packages.lock.json"));
	}
	await rejectInvalidLocks();
	process.stdout.write(`${JSON.stringify({ evidence, platform, bundledPrelude: values["bundled-prelude"], nativeExecution: true, aptPostinst: true, inheritedConfig: true, cacheRestore: true, inputInvalidation: true, commandInvalidation: true })}\n`);
} finally {
	if (initialized) await execute(engine, ["kill"], options);
	rmSync(root, { recursive: true, force: true });
}

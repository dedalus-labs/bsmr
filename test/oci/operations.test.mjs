//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Tests closed contexts and invalid OCI inputs before any encoder or worker runs.

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { chmod, lstat, mkdir, mkdtemp, readFile, readlink, realpath, rm, symlink, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { stageContext } from "../../prelude/oci/operations.mjs";

const operations = resolve(import.meta.dirname, "../../prelude/oci/operations.mjs");
const worker = resolve(import.meta.dirname, "../../prelude/oci/worker.mjs");

/** Own each real filesystem fixture and remove only that fixture afterward. */
async function fixture(t) {
	const root = await realpath(await mkdtemp(join(tmpdir(), "bsmr-oci-inputs-")));
	t.after(() => rm(root, { recursive: true, force: true }));
	return root;
}

/** Verify invalid layer inputs fail before invoking any encoder or writing outputs. */
async function invalidLayer(root, spec, code) {
	const specFile = join(root, "layer-spec.json");
	const metadata = join(root, "layer.json");
	const blob = join(root, "layer.tgz");
	await writeFile(specFile, JSON.stringify(spec));
	const result = spawnSync(process.execPath, [operations, "layer", "--img", join(root, "unavailable-encoder"),
		"--platform", "linux/arm64", "--spec", specFile, "--metadata", metadata, "--blob", blob],
	{ encoding: "utf8", timeout: 5000 });
	assert.equal(result.status, 1, result.stderr);
	assert.ok(result.stderr.startsWith(`${code}:`), result.stderr);
	assert.equal(existsSync(metadata), false);
	assert.equal(existsSync(blob), false);
}

test("staged contexts preserve bounded links with normalized private payloads", async (t) => {
	const root = await fixture(t);
	const source = join(root, "source");
	const output = join(root, "staged");
	await mkdir(join(source, "app"), { recursive: true });
	await writeFile(join(source, "app", "data"), "original\n");
	await writeFile(join(source, "app", "executable"), "executable\n");
	await chmod(join(source, "app", "data"), 0o600);
	await chmod(join(source, "app", "executable"), 0o711);
	await chmod(join(source, "app"), 0o700);
	await utimes(join(source, "app", "data"), 777, 777);
	await symlink("app/data", join(source, "current"));
	await symlink("../current", join(source, "app", "alias"));
	await stageContext(source, output, 1234);
	assert.equal(await readlink(join(output, "current")), "app/data");
	assert.equal(await readlink(join(output, "app", "alias")), "../current");
	assert.equal(await readFile(join(output, "app", "alias"), "utf8"), "original\n");
	for (const [path, mode] of [[output, 0o755], [join(output, "app"), 0o755],
		[join(output, "app", "data"), 0o644], [join(output, "app", "executable"), 0o755]]) {
		const stat = await lstat(path);
		assert.equal(stat.mode & 0o777, mode);
		assert.equal(stat.mtimeMs, 1_234_000);
	}
	const link = await lstat(join(output, "current"));
	assert.equal(link.isSymbolicLink(), true);
	assert.equal(link.mtimeMs, 1_234_000);
	const original = await lstat(join(source, "app", "data"));
	const staged = await lstat(join(output, "app", "data"));
	assert.equal(original.dev, staged.dev);
	assert.notEqual(original.ino, staged.ino);
	assert.equal(original.mode & 0o777, 0o600);
	assert.equal(original.mtimeMs, 777_000);
	await writeFile(join(output, "app", "data"), "changed\n");
	assert.equal(await readFile(join(source, "app", "data"), "utf8"), "original\n");
});

test("a context link cannot escape directly or through an outside link returning inside", async (t) => {
	const root = await fixture(t);
	const source = join(root, "source");
	const outside = join(root, "outside");
	await mkdir(source);
	await mkdir(outside);
	await writeFile(join(source, "data"), "inside\n");
	await writeFile(join(outside, "data"), "outside\n");
	await symlink("../outside/data", join(source, "link"));
	await assert.rejects(stageContext(source, join(root, "direct"), 0), { code: "OCI_EXTERNAL_CONTEXT_LINK" });
	await rm(join(source, "link"));
	await symlink("../source/data", join(outside, "back"));
	await symlink("../outside/back", join(source, "link"));
	assert.equal(await realpath(join(source, "link")), join(source, "data"));
	await assert.rejects(stageContext(source, join(root, "returning"), 0), { code: "OCI_EXTERNAL_CONTEXT_LINK" });
	await rm(join(source, "link"));
	await symlink(join(source, "data"), join(source, "link"));
	await assert.rejects(stageContext(source, join(root, "absolute"), 0), { code: "OCI_EXTERNAL_CONTEXT_LINK" });
});

test("context staging rejects nested destinations and parent aliases before creating output", async (t) => {
	const root = await fixture(t);
	const source = join(root, "source");
	await mkdir(source);
	await assert.rejects(stageContext(source, source, 0), { code: "OCI_CONTEXT_OVERLAP" });
	const nested = join(source, "staged");
	await assert.rejects(stageContext(source, nested, 0), { code: "OCI_CONTEXT_OVERLAP" });
	assert.equal(existsSync(nested), false);
	await symlink("source", join(root, "alias"));
	await assert.rejects(stageContext(source, join(root, "alias", "staged"), 0), { code: "OCI_CONTEXT_OVERLAP" });
	assert.equal(existsSync(nested), false);
});

test("context staging validates its directory and timestamp before creating output", async (t) => {
	const root = await fixture(t);
	const source = join(root, "source");
	const output = join(root, "output");
	await writeFile(source, "a file\n");
	await assert.rejects(stageContext(source, output, 0), { code: "OCI_INVALID_CONTEXT" });
	await assert.rejects(stageContext(root, output, -1), { code: "OCI_INVALID_TIMESTAMP" });
	assert.equal(existsSync(output), false);
});

test("context staging rejects a real FIFO instead of reading it", { skip: process.platform === "win32" }, async (t) => {
	const root = await fixture(t);
	const source = join(root, "source");
	await mkdir(source);
	execFileSync("mkfifo", [join(source, "pipe")]);
	await assert.rejects(stageContext(source, join(root, "staged"), 0), { code: "OCI_NONREGULAR_INPUT" });
});

test("interleaved destination prefixes cannot hide a file ancestor", async (t) => {
	const root = await fixture(t);
	const source = join(root, "data");
	await writeFile(source, "data\n");
	await invalidLayer(root, { files: { "/app": source, "/app-else": source, "/app/child": source },
		executables: {}, symlinks: {} }, "OCI_PLACEMENT_CONFLICT");
});

test("placements cannot duplicate a destination across file kinds", async (t) => {
	const root = await fixture(t);
	const source = join(root, "data");
	await writeFile(source, "data\n");
	await invalidLayer(root, { files: { "/app": source }, executables: {}, symlinks: { "/app": "other" } }, "OCI_PLACEMENT_CONFLICT");
});

test("an executable with the wrong ELF architecture fails before encoding", async (t) => {
	const root = await fixture(t);
	const source = join(root, "amd64-header");
	const header = Buffer.alloc(20);
	Buffer.from([127, 69, 76, 70, 2, 1]).copy(header);
	header.writeUInt16LE(62, 18);
	await writeFile(source, header);
	await invalidLayer(root, { files: {}, executables: { "/app": source }, symlinks: {} }, "OCI_EXECUTABLE_PLATFORM_MISMATCH");
});

test("placed sources must be regular leaves rather than directories or host symlinks", async (t) => {
	const root = await fixture(t);
	const directory = join(root, "directory");
	const source = join(root, "data");
	const link = join(root, "link");
	await mkdir(directory);
	await writeFile(source, "data\n");
	await symlink("data", link);
	for (const path of [directory, link]) {
		await invalidLayer(root, { files: { "/app": path }, executables: {}, symlinks: {} }, "OCI_NONREGULAR_INPUT");
	}
});

test("unsafe placement paths fail before reading sources or encoding", async (t) => {
	const root = await fixture(t);
	for (const path of ["app/file", "/app/../file", "/app//file", "/app\\file", "/app=bad", "/app/.wh.file"]) {
		await invalidLayer(root, { files: { [path]: join(root, "absent") }, executables: {}, symlinks: {} }, "OCI_INVALID_PATH");
	}
});

test("the qualified worker rejects nonlocal hosts and duplicated CLI options before invoking Docker", async (t) => {
	const root = await fixture(t);
	const contract = join(root, "daemon.json");
	const source = join(root, "source");
	const spec = join(root, "spec.json");
	await mkdir(source);
	await writeFile(spec, JSON.stringify({ context: source, dockerfile: "Dockerfile", platform: "linux/arm64",
		build_args: {}, target: null, source_date_epoch: 0 }));
	const args = [worker, "--docker", join(root, "absent-docker"), "--daemon-contract", contract,
		"--buildkit-image", `docker.io/moby/buildkit@sha256:${"0".repeat(64)}`, "--spec", spec, "--output", join(root, "output")];
	for (const host of ["tcp://localhost:2375", "ssh://local", "unix://relative/path"]) {
		await writeFile(contract, JSON.stringify({ host, version: "fixture", api_version: "1", os: "linux", architecture: "arm64", kernel_version: "fixture" }));
		const result = spawnSync(process.execPath, args, { encoding: "utf8", timeout: 5000 });
		assert.equal(result.status, 1, result.stderr);
		assert.ok(result.stderr.startsWith("UnsupportedDockerHost:"), result.stderr);
		assert.equal(existsSync(join(root, "output")), false);
	}
	const duplicated = spawnSync(process.execPath, [...args, "--docker", "another-client"], { encoding: "utf8", timeout: 5000 });
	assert.equal(duplicated.status, 1, duplicated.stderr);
	assert.ok(duplicated.stderr.startsWith("InvalidWorkerArguments:"), duplicated.stderr);
});

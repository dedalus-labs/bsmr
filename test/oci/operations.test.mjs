//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Rejects unsafe layer placements before the encoder reads their payloads.

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const operations = resolve(import.meta.dirname, "../../prelude/oci/operations.mjs");

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
	const compact = join(root, "layer.cstream");
	await writeFile(specFile, JSON.stringify(spec));
	const result = spawnSync(process.execPath, [operations, "layer", "--img", join(root, "unavailable-encoder"),
		"--platform", "linux/arm64", "--spec", specFile, "--metadata", metadata, "--compact", compact],
	{ encoding: "utf8", timeout: 5000 });
	assert.equal(result.status, 1, result.stderr);
	assert.ok(result.stderr.startsWith(`${code}:`), result.stderr);
	assert.equal(existsSync(metadata), false);
	assert.equal(existsSync(compact), false);
}

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

test("invariant source symlinks cannot stand in for declared files or directories", async (t) => {
	const root = await fixture(t);
	const directory = join(root, "directory");
	const source = join(root, "data");
	const link = join(root, "link");
	await mkdir(directory);
	await writeFile(source, "data\n");
	await symlink("data", link);
	await invalidLayer(root, { files: { "/app": link }, executables: {}, symlinks: {} }, "OCI_NONREGULAR_INPUT");
	await rm(link);
	await symlink("directory", link);
	await invalidLayer(root, { files: { "/app": link }, executables: {}, symlinks: {} }, "OCI_NONREGULAR_INPUT");
});

test("invariant directory layers reject special files before invoking the encoder", { skip: process.platform === "win32" }, async (t) => {
	const root = await fixture(t);
	const directory = join(root, "directory");
	await mkdir(directory);
	execFileSync("mkfifo", [join(directory, "pipe")]);
	await invalidLayer(root, { files: { "/app": directory }, executables: {}, symlinks: {} }, "OCI_NONREGULAR_INPUT");
});

test("unsafe placement paths fail before reading sources or encoding", async (t) => {
	const root = await fixture(t);
	for (const path of ["app/file", "/app/../file", "/app//file", "/app\\file", "/app=bad", "/app/.wh.file"]) {
		await invalidLayer(root, { files: { [path]: join(root, "absent") }, executables: {}, symlinks: {} }, "OCI_INVALID_PATH");
	}
});

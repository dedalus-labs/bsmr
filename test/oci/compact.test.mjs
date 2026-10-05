//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves deferred native layers retain their exact bytes and bounded input closure.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { chmod, lstat, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { gunzipSync, gzipSync } from "node:zlib";

const execute = promisify(execFile);
const encoder = process.env.BSMR_OCI_IMG;
assert.ok(encoder, "set BSMR_OCI_IMG to the pinned rules_img v0.3.22 executable");
const operation = resolve(import.meta.dirname, "../../prelude/oci/operations.mjs");

/** Run a declared production operation against the actual pinned encoder. */
async function invoke(command, values) {
	return execute(process.execPath, [operation, command, ...Object.entries(values).flatMap(([k, v]) => [`--${k}`, v])],
		{ timeout: 60_000, maxBuffer: 4 * 1024 * 1024 });
}

/** Own source, metadata, compact stream and explicitly requested full blob paths. */
async function fixture(t) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-oci-compact-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	const source = join(root, "source");
	await writeFile(source, "native payload\n".repeat(4096));
	const values = { img: encoder, platform: "linux/arm64", spec: join(root, "spec.json"),
		metadata: join(root, "layer.json"), compact: join(root, "layer.cstream") };
	await writeFile(values.spec, JSON.stringify({ files: { "/app/data": source }, executables: {}, symlinks: {} }));
	return { root, source, values, blob: join(root, "layer.tgz") };
}

test("invariant compact native layers reconstruct the original digest and diffID without retaining a full blob", async (t) => {
	const f = await fixture(t);
	await invoke("layer", f.values);
	assert.equal(existsSync(f.blob), false);
	const metadata = JSON.parse(await readFile(f.values.metadata, "utf8"));
	await invoke("materialize", { ...f.values, blob: f.blob });
	const actual = await readFile(f.blob);
	assert.equal(`sha256:${createHash("sha256").update(actual).digest("hex")}`, metadata.digest);
	assert.equal(`sha256:${createHash("sha256").update(gunzipSync(actual)).digest("hex")}`, metadata.diff_id);
	const original = join(f.root, "original.tgz");
	await execute(encoder, ["layer", "--format", "gzip", "--compression-level", "1", "--compressor-jobs", "1",
		"--create-parent-directories", "--default-metadata", JSON.stringify({ uid: 0, gid: 0, uname: "", gname: "", mtime: "1970-01-01T00:00:00Z" }),
		"--add", `/app/data=${f.source}`, "--file-metadata", 'app/data={"mode":"0644"}', original]);
	assert.deepEqual(actual, await readFile(original));
});

test("invariant missing or changed retained payloads cannot materialize an old compact layer", async (t) => {
	const f = await fixture(t);
	await invoke("layer", f.values);
	await writeFile(f.source, "different payload\n");
	await assert.rejects(invoke("materialize", { ...f.values, blob: f.blob }));
	assert.equal(existsSync(f.blob), false);
	await rm(f.source);
	await assert.rejects(invoke("materialize", { ...f.values, blob: f.blob }));
	assert.equal(existsSync(f.blob), false);
});

test("invariant payload edits change layer identities and corrupt compact streams fail closed", async (t) => {
	const f = await fixture(t);
	await invoke("layer", f.values);
	const before = JSON.parse(await readFile(f.values.metadata, "utf8"));
	await writeFile(f.source, "edited source\n");
	const edited = { ...f.values, metadata: join(f.root, "edited.json"), compact: join(f.root, "edited.cstream") };
	await invoke("layer", edited);
	const after = JSON.parse(await readFile(edited.metadata, "utf8"));
	assert.notEqual(before.digest, after.digest);
	assert.notEqual(before.diff_id, after.diff_id);
	await writeFile(edited.compact, "corrupt stream\n");
	await assert.rejects(invoke("materialize", { ...edited, blob: f.blob }), (error) => /OCI_ENCODER_FAILED/u.test(error.stderr));
	assert.equal(existsSync(f.blob), false);
});

test("invariant imported archives retain exact compressed bytes and Linux metadata", async (t) => {
	const f = await fixture(t);
	const archive = join(f.root, "original.tar");
	await execute("python3", ["-c", [
		"import io,sys,tarfile",
		"with tarfile.open(sys.argv[1], 'w', format=tarfile.PAX_FORMAT) as archive:",
		"  entry=tarfile.TarInfo('owned'); entry.size=7; entry.uid=123; entry.gid=456; entry.mode=0o4751; entry.mtime=1234; entry.pax_headers={'SCHILY.xattr.user.fixture':'preserved'}; archive.addfile(entry,io.BytesIO(b'payload'))",
		"  link=tarfile.TarInfo('linked'); link.type=tarfile.LNKTYPE; link.linkname='owned'; archive.addfile(link)",
		"  link=tarfile.TarInfo('absolute'); link.type=tarfile.SYMTYPE; link.linkname='/owned'; archive.addfile(link)",
	].join("\n"), archive]);
	const original = await readFile(archive);
	for (const [name, payload] of [["tar", original], ["gzip", gzipSync(original)]]) {
		const path = join(f.root, name + ".layer");
		const metadata = join(f.root, name + ".json");
		await writeFile(path, payload);
		await invoke("layer-from-tar", { img: encoder, platform: "linux/arm64", archive: path, metadata });
		const value = JSON.parse(await readFile(metadata, "utf8"));
		assert.equal(value.digest, `sha256:${createHash("sha256").update(payload).digest("hex")}`);
		assert.equal(value.diff_id, `sha256:${createHash("sha256").update(original).digest("hex")}`);
		assert.deepEqual(await readFile(path), payload);
	}
});

for (const compressed of [false, true]) {
	test(`invariant ${compressed ? "gzip" : "raw"} layer archives contain a readable tar`, async (t) => {
		const f = await fixture(t);
		const invalid = Buffer.from("this is not a tar archive\n".repeat(64));
		const payload = compressed ? gzipSync(invalid) : invalid;
		await writeFile(f.source, payload);
		await assert.rejects(invoke("layer-from-tar", { img: encoder, platform: "linux/arm64", archive: f.source, metadata: f.values.metadata }),
			(error) => /OCI_ENCODER_FAILED/u.test(error.stderr));
		assert.equal(existsSync(f.values.metadata), false);
		assert.deepEqual(await readFile(f.source), payload);
	});
}

test("invariant symlink-only and empty-directory layers reconstruct without CAS payloads", async (t) => {
	for (const kind of ["symlink", "directory"]) {
		const f = await fixture(t);
		const directory = join(f.root, "empty");
		await mkdir(directory);
		const spec = kind === "symlink" ? { files: {}, executables: {}, symlinks: { "/entry": "/missing" } }
			: { files: { "/entry": directory }, executables: {}, symlinks: {} };
		await writeFile(f.values.spec, JSON.stringify(spec));
		await invoke("layer", f.values);
		await invoke("materialize", { ...f.values, blob: f.blob });
		const metadata = JSON.parse(await readFile(f.values.metadata, "utf8"));
		const blob = await readFile(f.blob);
		assert.equal(`sha256:${createHash("sha256").update(blob).digest("hex")}`, metadata.digest);
		assert.equal(`sha256:${createHash("sha256").update(gunzipSync(blob)).digest("hex")}`, metadata.diff_id);
		const { stdout } = await execute("python3", ["-c", "import json,sys,tarfile; print(json.dumps({m.name:('symlink' if m.issym() else 'directory' if m.isdir() else 'file') for m in tarfile.open(sys.argv[1])}))", f.blob]);
		assert.deepEqual(JSON.parse(stdout), { entry: kind });
	}
});

test("invariant failed materialization never removes or overwrites an existing output", async (t) => {
	const f = await fixture(t);
	await invoke("layer", f.values);
	await writeFile(f.blob, "existing output\n");
	await assert.rejects(invoke("materialize", { ...f.values, blob: f.blob }), (error) => /EEXIST/u.test(error.stderr));
	await writeFile(f.values.compact, "corrupt\n");
	await assert.rejects(invoke("materialize", { ...f.values, blob: f.blob }), (error) => /OCI_ENCODER_FAILED/u.test(error.stderr));
	assert.equal(await readFile(f.blob, "utf8"), "existing output\n");
});

test("invariant directory layers preserve every entry and literal target symlinks without following host paths", async (t) => {
	const f = await fixture(t);
	const directory = join(f.root, "tree");
	await mkdir(join(directory, "empty"), { recursive: true });
	await writeFile(join(directory, "data"), "directory payload\n");
	await writeFile(join(directory, "script"), "#!/bin/sh\nexit 0\n");
	await chmod(join(directory, "script"), 0o755);
	await symlink("/etc/passwd", join(directory, "absolute"));
	await symlink("../source", join(directory, "outside"));
	await symlink("missing", join(directory, "dangling"));
	await writeFile(f.values.spec, JSON.stringify({ files: { "/app": directory }, executables: {}, symlinks: {} }));
	await invoke("layer", f.values);
	await invoke("materialize", { ...f.values, blob: f.blob });
	const extracted = join(f.root, "extracted");
	await mkdir(extracted);
	await execute("tar", ["-xf", f.blob, "-C", extracted]);
	assert.equal(await readFile(join(extracted, "app/data"), "utf8"), "directory payload\n");
	assert.equal(existsSync(join(extracted, "app/empty")), true);
	assert.equal((await lstat(join(extracted, "app/script"))).mode & 0o777, 0o755);
	const { stdout } = await execute("python3", ["-c", "import json,sys,tarfile; print(json.dumps({m.name:m.linkname for m in tarfile.open(sys.argv[1]) if m.issym()}))", f.blob]);
	assert.deepEqual(JSON.parse(stdout), { "app/absolute": "/etc/passwd", "app/outside": "../source", "app/dangling": "missing" });
});

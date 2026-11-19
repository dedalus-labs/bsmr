//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves exact OCI closure identities and independent exported payload ownership.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, mkdtemp, readFile, rename, rm, stat, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { gzipSync } from "node:zlib";
import { exportLayout, imageMetadata, importLayout, layerMetadata } from "../../prelude/oci/closure.mjs";

const manifestType = "application/vnd.oci.image.manifest.v1+json";
const configType = "application/vnd.oci.image.config.v1+json";
const layerType = "application/vnd.oci.image.layer.v1.tar+gzip";
const execute = promisify(execFile);

/** Encode one exact JSON artifact used by descriptor identities. */
function encode(value) { return Buffer.from(JSON.stringify(value)); }

/** Name the exact bytes independently of the validator implementation. */
function digest(bytes) { return `sha256:${createHash("sha256").update(bytes).digest("hex")}`; }

/** Build an image over a valid empty tar without needing a second tar writer. */
async function fixture(t, { diffID, architecture = "arm64" } = {}) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-oci-closure-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	const tar = Buffer.alloc(1024);
	const compressed = gzipSync(tar, { level: 1 });
	const layer = { mediaType: layerType, digest: digest(compressed), size: compressed.length, diff_id: diffID ?? digest(tar) };
	const config = encode({ os: "linux", architecture, variant: architecture === "arm64" ? "v8" : undefined,
		rootfs: { type: "layers", diff_ids: [layer.diff_id] }, config: { Entrypoint: ["/app/probe"] } });
	const configDescriptor = { mediaType: configType, digest: digest(config), size: config.length };
	const manifest = encode({ schemaVersion: 2, mediaType: manifestType, config: configDescriptor,
		layers: [{ mediaType: layer.mediaType, digest: layer.digest, size: layer.size }] });
	const descriptor = { mediaType: manifestType, digest: digest(manifest), size: manifest.length,
		platform: { os: "linux", architecture, ...(architecture === "arm64" ? { variant: "v8" } : {}) } };
	const paths = Object.fromEntries(["manifest", "config", "descriptor", "metadata", "blob"].map((name) => [name, join(root, name)]));
	await Promise.all([
		writeFile(paths.manifest, manifest), writeFile(paths.config, config), writeFile(paths.descriptor, encode(descriptor)),
		writeFile(paths.metadata, encode(layer)), writeFile(paths.blob, compressed),
	]);
	const spec = { manifest: paths.manifest, config: paths.config, descriptor: paths.descriptor,
		layers: [{ metadata: paths.metadata, blob: paths.blob }], base_layouts: [] };
	return { root, paths, spec, compressed, layer, descriptor };
}

/** Allocate fresh metadata outputs for an import invocation. */
function imports(root, layout) {
	return { platform: "linux/arm64", layout, manifest: join(root, "import-manifest"), config: join(root, "import-config"),
		descriptor: join(root, "import-descriptor") };
}

test("image composition reads only metadata and preserves exact descriptor identity", async (t) => {
	const f = await fixture(t);
	await rm(f.paths.blob);
	const result = await imageMetadata({ platform: "linux/arm64", ...f.spec });
	assert.deepEqual(result.descriptor, f.descriptor);
	assert.deepEqual(result.config.rootfs.diff_ids, [f.layer.diff_id]);
	assert.equal((await layerMetadata(f.paths.metadata)).diff_id, f.layer.diff_id);
});

test("complete exports and imports preserve bytes without hardlink aliases", async (t) => {
	const f = await fixture(t);
	const output = join(f.root, "layout");
	await exportLayout({ platform: "linux/arm64/v8", spec: f.spec, output });
	const exportedBlob = join(output, "blobs/sha256", f.layer.digest.slice(7));
	assert.deepEqual(await readFile(exportedBlob), f.compressed);
	assert.notEqual((await stat(exportedBlob)).ino, (await stat(f.paths.blob)).ino);
	const imported = await importLayout(imports(f.root, output));
	assert.deepEqual(imported.descriptor, f.descriptor);
	assert.deepEqual(await readFile(join(f.root, "import-manifest")), await readFile(f.paths.manifest));
	const alias = join(f.root, "layout-alias");
	await symlink(output, alias);
	assert.equal((await importLayout(imports(join(f.root, "new-parent"), alias))).descriptor.digest, f.descriptor.digest);
	await writeFile(exportedBlob, "corrupt export");
	assert.deepEqual(await readFile(f.paths.blob), f.compressed, "editing an export must not mutate a retained layer");
	await assert.rejects(importLayout({ ...imports(f.root, output), manifest: join(f.root, "retry-manifest") }),
		(error) => ["OCI_INVALID_LAYER", "OCI_DIGEST_MISMATCH"].includes(error.code));
});

test("exports restore base content by digest without copying unrelated blobs", async (t) => {
	const f = await fixture(t);
	const base = join(f.root, "base");
	await exportLayout({ platform: "linux/arm64", spec: f.spec, output: base });
	await writeFile(join(base, "blobs/sha256", "a".repeat(64)), "unreferenced");
	const output = join(f.root, "derived");
	await exportLayout({ platform: "linux/arm64", spec: { ...f.spec, layers: [], base_layouts: [base] }, output });
	await assert.rejects(lstat(join(output, "blobs/sha256", "a".repeat(64))), { code: "ENOENT" });
	assert.equal((await importLayout(imports(f.root, output))).descriptor.digest, f.descriptor.digest);
});

test("missing layers fail then an exact restored input succeeds", async (t) => {
	const f = await fixture(t);
	await rm(f.paths.blob);
	const output = join(f.root, "retry-layout");
	await assert.rejects(exportLayout({ platform: "linux/arm64", spec: f.spec, output }), { code: "OCI_MISSING_BLOB" });
	await assert.rejects(lstat(output), { code: "ENOENT" });
	await writeFile(f.paths.blob, f.compressed);
	await exportLayout({ platform: "linux/arm64", spec: f.spec, output });
	assert.equal((await importLayout(imports(f.root, output))).descriptor.digest, f.descriptor.digest);
});

test("corrupt compressed bytes and mismatched uncompressed diffIDs fail closed", async (t) => {
	const f = await fixture(t);
	await writeFile(f.paths.blob, gzipSync(Buffer.alloc(1024, 1), { level: 1 }));
	await assert.rejects(exportLayout({ platform: "linux/arm64", spec: f.spec, output: join(f.root, "corrupt") }),
		{ code: "OCI_DIGEST_MISMATCH" });
	const wrongDiffID = await fixture(t, { diffID: `sha256:${"0".repeat(64)}` });
	await assert.rejects(exportLayout({ platform: "linux/arm64", spec: wrongDiffID.spec, output: join(wrongDiffID.root, "wrong-diff") }),
		{ code: "OCI_DIFFID_MISMATCH" });
});

test("blob leaf symlinks are rejected while tracked ancestor symlinks remain usable", async (t) => {
	const f = await fixture(t);
	const alias = join(f.root, "alias");
	await symlink(f.root, alias);
	assert.equal((await layerMetadata(join(alias, "metadata"))).digest, f.layer.digest);
	const leaf = join(f.root, "leaf");
	await symlink(f.paths.blob, leaf);
	await assert.rejects(exportLayout({ platform: "linux/arm64", output: join(f.root, "symlink"),
		spec: { ...f.spec, layers: [{ metadata: f.paths.metadata, blob: leaf }] } }), { code: "OCI_INVALID_BLOB" });
});

test("layout algorithm-directory symlinks cannot supply an external undeclared closure", async (t) => {
	const f = await fixture(t);
	const output = join(f.root, "layout");
	await exportLayout({ platform: "linux/arm64", spec: f.spec, output });
	const external = join(f.root, "external-blobs");
	await rename(join(output, "blobs/sha256"), external);
	await symlink(external, join(output, "blobs/sha256"));
	await assert.rejects(importLayout(imports(f.root, output)), { code: "OCI_INVALID_LAYOUT" });
	await assert.rejects(exportLayout({ platform: "linux/arm64", output: join(f.root, "derived"),
		spec: { ...f.spec, layers: [], base_layouts: [output] } }), { code: "OCI_INVALID_LAYOUT" });
});

test("metadata allocation is bounded and gzip checksum errors are named honestly", async (t) => {
	const f = await fixture(t);
	await writeFile(f.paths.metadata, Buffer.alloc(16 * 1024 * 1024 + 1));
	await assert.rejects(layerMetadata(f.paths.metadata), { code: "OCI_METADATA_TOO_LARGE" });
	await writeFile(f.paths.metadata, encode(f.layer));
	const corrupt = Buffer.from(f.compressed);
	corrupt[corrupt.length - 1] ^= 1;
	await writeFile(f.paths.blob, corrupt);
	await assert.rejects(exportLayout({ platform: "linux/arm64", spec: f.spec, output: join(f.root, "crc") }),
		(error) => error.code === "OCI_INVALID_LAYER" && /checksum/u.test(error.message));
});

test("FIFO metadata and blob leaves fail promptly before any blocking read", { skip: process.platform === "win32" }, async (t) => {
	const f = await fixture(t);
	const fifo = join(f.root, "fifo");
	await execute("mkfifo", [fifo]);
	const script = `const {layerMetadata, exportLayout} = await import(process.argv[1]);
try {
  if (process.argv[2] === "metadata") await layerMetadata(process.argv[3]);
  else await exportLayout({platform:"linux/arm64", spec:JSON.parse(process.argv[3]), output:process.argv[4]});
  process.exitCode = 2;
} catch (error) { process.stderr.write(error.code + "\\n"); process.exitCode = 1; }`;
	const module = new URL("../../prelude/oci/closure.mjs", import.meta.url).href;
	for (const args of [["metadata", fifo], ["blob", JSON.stringify({ ...f.spec,
		layers: [{ metadata: f.paths.metadata, blob: fifo }] }), join(f.root, "fifo-layout")]]) {
		await assert.rejects(execute(process.execPath, ["--input-type=module", "-e", script, module, ...args],
			{ timeout: 2000, killSignal: "SIGKILL" }), (error) => error.code === 1 && !error.killed
				&& /OCI_INVALID_BLOB/u.test(error.stderr), "special-file rejection must complete without a child timeout");
	}
});

test("descriptor digests, sizes, target platforms and config identities are checked", async (t) => {
	const f = await fixture(t);
	await assert.rejects(imageMetadata({ platform: "linux/amd64", ...f.spec }), { code: "OCI_PLATFORM_MISMATCH" });
	await writeFile(f.paths.descriptor, encode({ ...f.descriptor, size: Number.MAX_SAFE_INTEGER + 1 }));
	await assert.rejects(imageMetadata({ platform: "linux/arm64", ...f.spec }), { code: "OCI_INVALID_DESCRIPTOR" });
	await writeFile(f.paths.descriptor, encode(f.descriptor));
	await writeFile(f.paths.config, "{}");
	await assert.rejects(imageMetadata({ platform: "linux/arm64", ...f.spec }), { code: "OCI_DIGEST_MISMATCH" });
});

test("existing outputs are never overwritten or removed", async (t) => {
	const f = await fixture(t);
	const output = join(f.root, "existing");
	await mkdir(output);
	await writeFile(join(output, "retained"), "keep");
	await assert.rejects(exportLayout({ platform: "linux/arm64", spec: f.spec, output }), { code: "EEXIST" });
	assert.equal(await readFile(join(output, "retained"), "utf8"), "keep");
});

test("imports reject nested indexes explicitly", async (t) => {
	const f = await fixture(t);
	const layout = join(f.root, "nested");
	await mkdir(join(layout, "blobs/sha256"), { recursive: true });
	await writeFile(join(layout, "oci-layout"), '{"imageLayoutVersion":"1.0.0"}');
	await writeFile(join(layout, "index.json"), encode({ schemaVersion: 2,
		manifests: [{ ...f.descriptor, mediaType: "application/vnd.oci.image.index.v1+json" }] }));
	await assert.rejects(importLayout(imports(f.root, layout)), { code: "OCI_NESTED_INDEX_UNSUPPORTED" });
});

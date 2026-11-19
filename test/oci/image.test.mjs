//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies real img layer composition and Docker-compatible base config semantics.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { imageMetadata, importLayout, layerMetadata } from "../../prelude/oci/closure.mjs";

const execute = promisify(execFile);
const encoder = process.env.BSMR_OCI_IMG;
assert.ok(encoder, "set BSMR_OCI_IMG to the pinned rules_img v0.3.22 executable");
const operation = resolve(import.meta.dirname, "../../prelude/oci/operations.mjs");

/** Invoke the production wrapper with declared paths and no shell interpolation. */
async function invoke(command, values) {
	const args = Object.entries(values).flatMap(([key, value]) => [`--${key}`, value]);
	return execute(process.execPath, [operation, command, ...args], { timeout: 60_000, maxBuffer: 4 * 1024 * 1024 });
}

/** Persist the exact machine recipe consumed by one operation. */
async function specification(root, name, value) {
	const path = join(root, `${name}.json`);
	await writeFile(path, JSON.stringify(value));
	return path;
}

/** Allocate fresh image outputs and preserve the public operation's argument names. */
function outputs(root, name) {
	return { manifest: join(root, name, "manifest.json"), config: join(root, name, "config.json"),
		descriptor: join(root, name, "descriptor.json") };
}

/** Supply every config field explicitly so null inheritance remains observable. */
function imageSpec(base, layers, overrides = {}) {
	return { base_manifest: base?.manifest ?? null, base_config: base?.config ?? null, base_descriptor: base?.descriptor ?? null,
		layers, entrypoint: null, cmd: null, env: {}, labels: {}, user: null, working_dir: null, ...overrides };
}

/** Build two real deterministic layers and a configured base image. */
async function fixture(t) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-oci-image-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	await mkdir(join(root, "sources"));
	const layers = [];
	for (const [name, imagePath] of [["base", "/runtime/base.txt"], ["app", "/app/extra.txt"]]) {
		const input = join(root, "sources", name);
		await writeFile(input, `${name} content\n`);
		const metadata = join(root, `${name}-layer.json`);
		const blob = join(root, `${name}-layer.tgz`);
		const spec = await specification(root, `${name}-placements`, { files: { [imagePath]: input }, executables: {}, symlinks: {} });
		await invoke("layer", { img: encoder, platform: "linux/arm64", spec, metadata, blob });
		layers.push({ metadata, blob, value: await layerMetadata(metadata) });
	}
	const base = outputs(root, "base");
	const spec = await specification(root, "base-image", imageSpec(null, [layers[0].metadata], {
		entrypoint: ["/bin/base"], cmd: ["--mode", "base"], env: { A: "one", KEEP: "present" },
		labels: { stable: "base" }, user: "1000:1000", working_dir: "/runtime",
	}));
	await invoke("image", { img: encoder, platform: "linux/arm64", spec, ...base });
	return { root, layers, base };
}

/** Compose one extra layer using the real base-metadata preparation path. */
async function derived(f, name, overrides = {}) {
	const result = outputs(f.root, name);
	const spec = await specification(f.root, `${name}-spec`, imageSpec(f.base, [f.layers[1].metadata], overrides));
	await invoke("image", { img: encoder, platform: "linux/arm64", spec, ...result });
	return { paths: result, metadata: await imageMetadata({ platform: "linux/arm64", ...result }) };
}

/** Construct a valid imported-base identity with explicitly controlled optional history. */
async function baseHistory(f, history) {
	const config = JSON.parse(await readFile(f.base.config, "utf8"));
	if (history === undefined) delete config.history;
	else config.history = history;
	const configBytes = Buffer.from(JSON.stringify(config));
	const manifest = JSON.parse(await readFile(f.base.manifest, "utf8"));
	manifest.config.digest = `sha256:${createHash("sha256").update(configBytes).digest("hex")}`;
	manifest.config.size = configBytes.length;
	const manifestBytes = Buffer.from(JSON.stringify(manifest));
	const descriptor = JSON.parse(await readFile(f.base.descriptor, "utf8"));
	descriptor.digest = `sha256:${createHash("sha256").update(manifestBytes).digest("hex")}`;
	descriptor.size = manifestBytes.length;
	await Promise.all([writeFile(f.base.config, configBytes), writeFile(f.base.manifest, manifestBytes),
		writeFile(f.base.descriptor, JSON.stringify(descriptor))]);
}

test("real image composition preserves base layers, ordered diffIDs and inherited config", async (t) => {
	const f = await fixture(t);
	const result = await derived(f, "inherited");
	assert.deepEqual(result.metadata.manifest.layers.map(({ digest }) => digest), f.layers.map(({ value }) => value.digest));
	assert.deepEqual(result.metadata.config.rootfs.diff_ids, f.layers.map(({ value }) => value.diff_id));
	const base = await imageMetadata({ platform: "linux/arm64", ...f.base });
	assert.equal(base.config.history.filter((entry) => !entry.empty_layer).length, 1);
	assert.deepEqual(result.metadata.config.history, [...base.config.history, ...f.layers[1].value.history],
		"base history must remain aligned with the base and application filesystem layers");
	const config = result.metadata.config.config;
	assert.equal(config.User, "1000:1000");
	assert.equal(config.WorkingDir, "/runtime");
	assert.deepEqual(config.Entrypoint, ["/bin/base"]);
	assert.deepEqual(config.Cmd, ["--mode", "base"]);
	assert.deepEqual(config.Env, ["A=one", "KEEP=present"]);
	assert.deepEqual(config.Labels, { stable: "base" });
});

test("real img supports explicit clearing and setting entrypoint clears inherited cmd", async (t) => {
	const f = await fixture(t);
	const cleared = (await derived(f, "cleared", { entrypoint: [], cmd: [], user: "", working_dir: "" })).metadata.config.config;
	assert.deepEqual(cleared.Entrypoint ?? [], []);
	assert.deepEqual(cleared.Cmd ?? [], []);
	assert.equal(cleared.User ?? "", "");
	assert.equal(cleared.WorkingDir ?? "", "");
	const replaced = (await derived(f, "replaced", { entrypoint: ["/app/extra"],
		env: { A: "two", NEW: "new" }, labels: { stable: "changed", added: "yes" } })).metadata.config.config;
	assert.deepEqual(replaced.Entrypoint, ["/app/extra"]);
	assert.deepEqual(replaced.Cmd ?? [], [], "setting entrypoint must clear the base command under Docker semantics");
	assert.deepEqual(replaced.Env, ["A=two", "KEEP=present", "NEW=new"]);
	assert.deepEqual(replaced.Labels, { stable: "changed", added: "yes" });
});

test("base empty history entries preserve order and absent history gets explicit placeholders", async (t) => {
	const f = await fixture(t);
	const history = [{ created_by: "ENV BASE=one", empty_layer: true }, { created_by: "COPY /runtime/base.txt" },
		{ created_by: "LABEL base=one", empty_layer: true }];
	await baseHistory(f, history);
	const preserved = (await derived(f, "history")).metadata;
	assert.deepEqual(preserved.config.history, [...history, ...f.layers[1].value.history]);
	assert.equal(preserved.config.history.filter((entry) => !entry.empty_layer).length, preserved.manifest.layers.length);
	await baseHistory(f, undefined);
	const missing = (await derived(f, "missing-history")).metadata;
	assert.deepEqual(missing.config.history, [{ created_by: "history missing" }, ...f.layers[1].value.history]);
});

test("complete derived export retains base bytes from its declared layout", async (t) => {
	const f = await fixture(t);
	const baseLayout = join(f.root, "base-layout");
	const baseSpec = await specification(f.root, "base-export", { ...f.base,
		layers: [{ metadata: f.layers[0].metadata, blob: f.layers[0].blob }], base_layouts: [] });
	await invoke("layout", { platform: "linux/arm64", spec: baseSpec, output: baseLayout });
	const result = await derived(f, "derived");
	const layout = join(f.root, "derived-layout");
	const spec = await specification(f.root, "derived-export", { ...result.paths,
		layers: [{ metadata: f.layers[1].metadata, blob: f.layers[1].blob }], base_layouts: [baseLayout] });
	await invoke("layout", { platform: "linux/arm64", spec, output: layout });
	const imported = await importLayout({ platform: "linux/arm64", layout, ...outputs(f.root, "imported") });
	assert.deepEqual(imported, result.metadata);
	for (const layer of f.layers) {
		assert.deepEqual(await readFile(join(layout, "blobs/sha256", layer.value.digest.slice(7))), await readFile(layer.blob));
	}
	await assert.rejects(importLayout({ platform: "linux/amd64", layout, ...outputs(f.root, "wrong-platform") }),
		{ code: "OCI_PLATFORM_SELECTION" });
});

test("base platform mismatch and tampered base manifest fail before img composition", async (t) => {
	const f = await fixture(t);
	const spec = await specification(f.root, "bad-base", imageSpec(f.base, [f.layers[1].metadata]));
	await assert.rejects(invoke("image", { img: encoder, platform: "linux/amd64", spec, ...outputs(f.root, "wrong") }),
		(error) => /OCI_PLATFORM_MISMATCH/u.test(error.stderr));
	await writeFile(f.base.manifest, "{}\n");
	await assert.rejects(invoke("image", { img: encoder, platform: "linux/arm64", spec, ...outputs(f.root, "tampered") }),
		(error) => /OCI_DIGEST_MISMATCH/u.test(error.stderr));
});

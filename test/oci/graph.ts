//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies compact OCI action dependencies and cache restoration with a released engine.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { parseArgs } from "node:util";
import { verifyBundledPrelude } from "./bundled.ts";
import { timedExec } from "./exec.ts";
import artifactRule from "./fixtures/artifact.bzl";
import graphBuild from "./fixtures/graph/recipe.bzl";
import archiveBuild from "./fixtures/graph/archive.bzl";
import treeScript from "./fixtures/graph/script.sh";
import verifyScript from "./verify.py";

const { positionals, values } = parseArgs({ allowPositionals: true, options: {
	platform: { type: "string", default: "linux/arm64" },
	"engine-version": { type: "string", default: "0.0.9" },
	artifacts: { type: "string" },
	"bundled-prelude": { type: "boolean", default: false },
} });
const [binary, img, prelude] = positionals;
assert.ok(binary && img && prelude && values.artifacts, "pass BSMR, img, source prelude, and --artifacts <evidence directory>");
const platform = values.platform;
assert.ok(platform === "linux/arm64" || platform === "linux/amd64");
mkdirSync(resolve(values.artifacts), { recursive: true });
const root = realpathSync(mkdtempSync(join(resolve(values.artifacts), "graph-")));
const cwd = join(root, "workspace");
const cache = join(root, "cache");
mkdirSync(cwd);
mkdirSync(cache);
const run = timedExec(120);
const executable = resolve(binary);
const env = { BSMR_LOCAL_CACHE_DIR: cache };
const options = { cwd, env };
let initialized = false;
let complete = false;
let bundledPreludeHash: string | undefined;
type Action = { identity: string; reproducer: { executor: string } };
type Receipt = { actions: Action[]; local: Action[]; cached: Action[] };
type Layer = { digest: string; diff_id: string };
type Image = { digest: string; layers: { digest: string }[]; files: Record<string, { type: string; mode: number; target?: string; sha256?: string }> };

/** Retain exact build reports and machine-readable action receipts for each phase. */
function build(phase: string, target: string): Promise<Receipt & { output: string }>;
function build(phase: string, target: string, runArgs: string[]): Promise<Receipt>;
async function build(phase: string, target: string, runArgs?: string[]) {
	const report = join(root, `${phase}-build.json`);
	try {
		await run(executable, [runArgs === undefined ? "build" : "run", `//:${target}`, "--build-report", report,
			"--console", "simple", "-v=1,stderr,full_failed_command", ...(runArgs === undefined ? [] : ["--", ...runArgs])], options);
	} catch (error) {
		writeFileSync(join(root, `${phase}-error.log`), String(error));
		throw error;
	}
	const result = JSON.parse(readFileSync(report, "utf8"));
	assert.equal(result.success, true);
	const log = await run(executable, ["log", "what-ran", "--trace-id", result.trace_id, "--format", "json"], options);
	const actions: Action[] = log.stdout.trim() ? log.stdout.trim().split("\n").map((line) => JSON.parse(line)) : [];
	writeFileSync(join(root, `${phase}-actions.json`), JSON.stringify(actions, null, 2) + "\n");
	const local = actions.filter((action) => action.reproducer.executor === "Local");
	const cached = actions.filter((action) => action.reproducer.executor === "Cache");
	process.stdout.write(JSON.stringify({ phase, trace: result.trace_id, actions: actions.length, local: local.length, cached: cached.length }) + "\n");
	if (runArgs !== undefined) return { actions, local, cached };
	const label = /^([^\[\]]+)(?:\[([^\[\]]+)\])?$/u.exec(target);
	assert.ok(label);
	const output = resolve(cwd, result.results[`root//:${label[1]}`].outputs[label[2] ?? "DEFAULT"][0]);
	return { actions, local, cached, output };
}

/** Inspect only the fixture's build artifacts, recording which payloads were retained. */
function artifacts(phase: string) {
	const directory = join(cwd, "bsmr-out/default/art");
	const paths = readdirSync(directory, { recursive: true, withFileTypes: true }).filter((entry) => entry.isFile())
		.map((entry) => relative(directory, join(entry.parentPath, entry.name))).sort();
	writeFileSync(join(root, `${phase}-artifacts.json`), JSON.stringify(paths, null, 2) + "\n");
	return paths;
}

/** Verify standard exported bytes using the independent standard-library tar reader. */
async function verify(path: string, phase: string): Promise<Image> {
	const result = await run("python3", ["-c", verifyScript, path], options);
	writeFileSync(join(root, `${phase}-image.json`), result.stdout);
	return JSON.parse(result.stdout);
}

/** Identify local actions by their public action category. */
function categories(actions: Action[]) {
	return actions.map(({ identity }) => identity.match(/\((oci_[a-z_]+)\)$/)?.[1]).filter((category) => category !== undefined).sort();
}

/** Record this fixture's daemon without starting one when none is running. */
async function daemon(phase: string) {
	const status = await run(executable, ["status"], options);
	const value = status.stdout.trim() ? JSON.parse(status.stdout) : null;
	const summary = value === null ? null : { project_root: value.project_root, forkserver_pid: value.forkserver_pid,
		process_info: { pid: value.process_info.pid, version: value.process_info.version } };
	writeFileSync(join(root, `${phase}-daemon.json`), JSON.stringify(summary, null, 2) + "\n");
	return summary;
}

try {
	const version = (await run(executable, ["--version"], { env })).stdout.trim();
	assert.equal(version, `bsmr ${values["engine-version"]}`);
	await run(executable, ["init"], options);
	initialized = true;
	if (values["bundled-prelude"]) bundledPreludeHash = await verifyBundledPrelude(executable, resolve(prelude), cwd, (file, args) => run(file, args, options), root);
	else {
		cpSync(resolve(prelude), join(cwd, "prelude"), { recursive: true });
		writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	cpSync(resolve(img), join(cwd, "img"));
	cpSync(process.execPath, join(cwd, "node"));
	writeFileSync(join(cwd, "defs.bzl"), artifactRule);
	writeFileSync(join(cwd, "payload.txt"), "native payload one\n");
	mkdirSync(join(cwd, "tree/empty"), { recursive: true });
	writeFileSync(join(cwd, "tree/data"), "directory content\n");
	writeFileSync(join(cwd, "tree/script"), treeScript);
	chmodSync(join(cwd, "tree/script"), 0o755);
	symlinkSync("/app/payload", join(cwd, "tree/current"));
	const definition = join(cwd, "BUILD.bsmr");
	writeFileSync(definition, graphBuild);
	const fixture = join(cwd, "fixture.json");
	writeFileSync(fixture, JSON.stringify({ platform, labels: { phase: "one" } }));
	const cold = await build("metadata-cold", "image");
	const originalDaemon = await daemon("metadata-cold");
	assert.ok(originalDaemon !== null);
	assert.equal(originalDaemon.project_root, cwd);
	assert.deepEqual(categories(cold.local), ["oci_image", "oci_layer", "oci_layer"]);
	const retained = artifacts("metadata-cold");
	assert.equal(retained.filter((path) => path.endsWith("/layer.cstream")).length, 2);
	assert.equal(retained.some((path) => path.endsWith(".tgz")), false);
	const layer = await build("layer-metadata", "layer");
	const original: Layer = JSON.parse(readFileSync(layer.output, "utf8"));
	assert.equal(layer.actions.length, 0);
	assert.equal((await build("metadata-warm", "image")).actions.length, 0);
	writeFileSync(fixture, JSON.stringify({ platform, labels: { phase: "two" } }));
	assert.deepEqual(categories((await build("configuration-edit", "image")).local), ["oci_image"]);
	assert.equal(artifacts("configuration-edit").some((path) => path.endsWith(".tgz")), false);
	const layout = await build("layout-cold", "layout");
	assert.deepEqual(categories(layout.local), ["oci_layout"]);
	const image = await verify(layout.output, "layout-cold");
	assert.equal(image.layers[0]?.digest, original.digest);
	assert.equal(image.files["tree/data"]?.sha256, createHash("sha256").update("directory content\n").digest("hex"));
	assert.equal(image.files["tree/script"]?.mode, 0o755);
	assert.equal(image.files["tree/current"]?.target, "/app/payload");
	const blob = await build("explicit-blob", "layer[blob]");
	assert.deepEqual(categories(blob.local), ["oci_materialize"]);
	assert.deepEqual(readFileSync(blob.output), readFileSync(join(layout.output, "blobs/sha256", original.digest.slice(7))));
	assert.equal((await build("blob-warm", "layer[blob]")).actions.length, 0);
	cpSync(blob.output, join(cwd, "archive.tgz"));
	writeFileSync(definition, graphBuild + archiveBuild);
	const archiveLayout = await build("archive-layout", "archive_layout");
	assert.deepEqual(categories(archiveLayout.local), ["oci_image", "oci_layer_from_tar", "oci_layout"]);
	assert.deepEqual((await verify(archiveLayout.output, "archive-layout")).files, image.files);
	assert.deepEqual(readFileSync(join(archiveLayout.output, "blobs/sha256", original.digest.slice(7))), readFileSync(join(cwd, "archive.tgz")));
	writeFileSync(join(cwd, "payload.txt"), "native payload two\n");
	assert.deepEqual(categories((await build("source-edit", "image")).local), ["oci_image", "oci_layer"]);
	const editedLayer = await build("edited-layer-metadata", "layer");
	const edited: Layer = JSON.parse(readFileSync(editedLayer.output, "utf8"));
	assert.notEqual(edited.digest, original.digest);
	assert.notEqual(edited.diff_id, original.diff_id);
	const editedLayout = await build("edited-layout", "layout");
	const editedImage = await verify(editedLayout.output, "edited-layout");
	await run(executable, ["clean"], options);
	assert.equal(existsSync(join(cwd, "bsmr-out/default/art")), false, "clean must remove this workspace's build artifacts");
	assert.equal(await daemon("after-clean"), null, "clean must stop the original daemon");
	const restored = await build("metadata-restored", "image");
	const restoredDaemon = await daemon("metadata-restored");
	assert.ok(restoredDaemon !== null);
	assert.notDeepEqual(restoredDaemon.process_info, originalDaemon.process_info);
	assert.deepEqual(categories(restored.local), []);
	assert.ok(restored.cached.length >= 3, "both layers and image metadata must restore from the independent cache");
	assert.equal(artifacts("metadata-restored").some((path) => path.endsWith(".tgz")), false);
	const restoredLayout = await build("layout-restored", "layout");
	assert.equal(restoredLayout.local.length, 0);
	assert.deepEqual(await verify(restoredLayout.output, "layout-restored"), editedImage);
	cpSync(restoredLayout.output, join(root, "verified-layout"), { recursive: true });
	const restoredBlob = await build("blob-after-clean", "layer[blob]");
	assert.deepEqual(categories(restoredBlob.local), ["oci_materialize"], "explicit blobs are reconstructed instead of duplicated in the action cache");
	assert.deepEqual(readFileSync(restoredBlob.output), readFileSync(join(restoredLayout.output, "blobs/sha256", edited.digest.slice(7))));
	assert.equal((await build("restored-warm", "layout")).actions.length, 0);
	const prepared = await build("publish-prepared", "publish");
	assert.deepEqual(categories(prepared.local), ["oci_push_layout", "oci_push_metadata"]);
	cpSync(prepared.output, join(root, "publish-request.json"));
	const published = join(root, "published-layout");
	assert.equal(existsSync(published), false);
	assert.equal((await build("publish-local", "publish", ["--sink", `oci:${published}`])).actions.length, 0);
	assert.deepEqual(await verify(published, "published"), editedImage);
	const publishedIndex = JSON.parse(readFileSync(join(published, "index.json"), "utf8"));
	assert.equal(publishedIndex.manifests[0].annotations["org.opencontainers.image.ref.name"], "example.invalid/team/image:test");
	const digestSink = join(root, "published-digest-layout");
	await build("publish-digest-local", "publish_digest", ["--sink", `oci:${digestSink}`]);
	assert.deepEqual(await verify(digestSink, "published-digest"), editedImage);
	const digestIndex = JSON.parse(readFileSync(join(digestSink, "index.json"), "utf8"));
	assert.equal(digestIndex.manifests[0].annotations["org.opencontainers.image.ref.name"], "example.invalid/team/image");
	const proofPrelude = values["bundled-prelude"] ? resolve(prelude) : join(cwd, "prelude");
	const inputs = Object.fromEntries(["img", "node", ...readdirSync(join(proofPrelude, "oci")).map((name) => `prelude/oci/${name}`)]
		.map((path) => [path, createHash("sha256").update(readFileSync(path.startsWith("prelude/") ? join(proofPrelude, path.slice(8)) : join(cwd, path))).digest("hex")]));
	writeFileSync(join(root, "receipt.json"), JSON.stringify({ engine: version, sourcePreludeOverlay: !values["bundled-prelude"], bundledPreludeHash, platform, inputs,
		originalLayer: original, editedLayer: edited, verifiedImage: editedImage.digest, root }, null, 2) + "\n");
	process.stdout.write(`ok: compact metadata, directories, archives, deferred blobs, cache restoration, local publication; evidence ${root}\n`);
	complete = true;
} finally {
	if (initialized) {
		await run(executable, ["kill"], options);
		assert.equal(await daemon("final"), null);
	}
	if (complete) {
		rmSync(cwd, { recursive: true });
		rmSync(cache, { recursive: true });
	} else process.stderr.write(`preserved failed fixture: ${root}\n`);
}

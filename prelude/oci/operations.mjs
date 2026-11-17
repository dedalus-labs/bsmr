//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Adapts declared native artifacts to pinned img operations and verified layouts.

import { execFile } from "node:child_process";
import { constants } from "node:fs";
import { copyFile, lstat, mkdir, mkdtemp, open, readFile, readdir, readlink, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs, promisify } from "node:util";
import { exportLayout, imageMetadata, importLayout, layerMetadata, verifyLayer } from "./closure.mjs";

const execute = promisify(execFile);
const inherit = "<inherit from base>";
const epoch = "1970-01-01T00:00:00Z";

class OciError extends Error {
	/** Name the invalid input boundary without selecting another implementation. */
	constructor(code, message) {
		super(message);
		this.code = code;
	}
}

/** Validate an owned recipe, rejecting missing and unknown fields. */
function object(value, fields, name) {
	if (value === null || typeof value !== "object" || Array.isArray(value)
		|| Object.keys(value).length !== fields.length || fields.some((key) => !Object.hasOwn(value, key))) {
		throw new OciError("OCI_INVALID_SPEC", name + " has an unsupported schema");
	}
	return value;
}

/** Require a process-safe string, preserving explicit empty config values. */
function string(value, name, empty = false) {
	if (typeof value !== "string" || (!empty && value === "") || /[\0\r\n]/u.test(value)) {
		throw new OciError("OCI_INVALID_SPEC", name + " must be a string without control delimiters");
	}
	return value;
}

/** Require a string map for configuration or declared placements. */
function stringMap(value, name) {
	if (value === null || typeof value !== "object" || Array.isArray(value)) {
		throw new OciError("OCI_INVALID_SPEC", name + " must be a string map");
	}
	for (const [key, entry] of Object.entries(value)) {
		string(key, name + " key");
		string(entry, name + " value", true);
	}
	return value;
}

/** Validate normalized destinations before creating any output path. */
function imagePath(path) {
	string(path, "image path");
	if (!path.startsWith("/") || /[=\\]/u.test(path) || path.trim() !== path
		|| path.slice(1).split("/").some((part) => ["", ".", ".."].includes(part) || part.startsWith(".wh."))) {
		throw new OciError("OCI_INVALID_PATH", "image path must be absolute and normalized: " + path);
	}
	return path;
}

/** Select an explicitly qualified Linux architecture, never the execution host. */
function architecture(platform) {
	if (!["linux/arm64", "linux/amd64"].includes(platform)) {
		throw new OciError("OCI_UNSUPPORTED_PLATFORM", "platform must be linux/arm64 or linux/amd64");
	}
	return platform.split("/")[1];
}

/** Check regular artifact leaves; ELF checks do not prove a dynamic runtime closure. */
async function regularFile(path, executable, platform) {
	string(path, "source artifact");
	if (!(await lstat(path)).isFile()) {
		throw new OciError("OCI_NONREGULAR_INPUT", "placement source must be a regular file: " + path);
	}
	if (executable) {
		const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW);
		try {
			const header = Buffer.alloc(20);
			const { bytesRead } = await file.read(header, 0, header.length, 0);
			const machine = architecture(platform) === "arm64" ? 183 : 62;
			if (bytesRead !== 20 || !header.subarray(0, 4).equals(Buffer.from([127, 69, 76, 70]))
				|| header[4] !== 2 || header[5] !== 1 || header.readUInt16LE(18) !== machine) {
				throw new OciError("OCI_EXECUTABLE_PLATFORM_MISMATCH", "executable must be ELF64 for " + platform + ": " + path);
			}
		} finally {
			await file.close();
		}
	}
}

/** Validate every native layer placement before invoking the encoder. */
async function placements(spec, platform) {
	object(spec, ["files", "executables", "symlinks"], "placements");
	architecture(platform);
	const entries = [];
	for (const kind of ["files", "executables", "symlinks"]) {
		for (const [path, source] of Object.entries(stringMap(spec[kind], kind))) {
			imagePath(path);
			string(source, "placement source");
			entries.push({ path, source, kind });
		}
	}
	entries.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
	if (entries.length === 0) throw new OciError("OCI_EMPTY_PLACEMENTS", "placement requires at least one entry");
	const seen = new Set();
	for (const { path } of entries) {
		if (seen.has(path)) throw new OciError("OCI_PLACEMENT_CONFLICT", "duplicate image entry: " + path);
		seen.add(path);
	}
	for (const { path } of entries) {
		for (let parent = dirname(path); parent !== "/"; parent = dirname(parent)) {
			if (seen.has(parent)) throw new OciError("OCI_PLACEMENT_CONFLICT", "placed file or symlink is an ancestor: " + parent);
		}
	}
	const expanded = [];
	/** Preserve target symlinks as text; never inspect their referents on the host. */
	async function tree(path, source) {
		imagePath(path);
		const info = await lstat(source);
		if (info.isDirectory()) {
			expanded.push({ path, source, kind: "directories" });
			for (const name of (await readdir(source)).sort()) await tree(path + "/" + name, join(source, name));
		} else if (info.isSymbolicLink()) {
			const target = await readlink(source);
			string(target, "directory symlink target");
			expanded.push({ path, source: target, kind: "symlinks" });
		} else if (info.isFile()) expanded.push({ path, source, kind: "files", mode: info.mode & 0o111 ? "0755" : "0644" });
		else throw new OciError("OCI_NONREGULAR_INPUT", "directory contains a special file: " + source);
	}
	for (const entry of entries) {
		if (entry.kind === "files" && (await lstat(entry.source)).isDirectory()) await tree(entry.path, entry.source);
		else {
			if (entry.kind !== "symlinks") await regularFile(entry.source, entry.kind === "executables", platform);
			expanded.push(entry);
		}
	}
	return expanded;
}

/** Make declared output parents without following an existing output leaf. */
async function parents(paths) {
	for (const path of paths) await mkdir(dirname(path), { recursive: true });
}

/** Run only the declared pinned encoder with bounded diagnostic output. */
async function img(binary, args) {
	try {
		await execute(resolve(binary), args, { maxBuffer: 4 * 1024 * 1024, timeout: 300_000,
			env: { LANG: "C", TZ: "UTC" } });
	} catch (error) {
		if (error.stderr) process.stderr.write(error.stderr);
		throw new OciError("OCI_ENCODER_FAILED", "pinned img operation failed");
	}
}

/** Retain only compact headers and references; the encoder discards full gzip bytes. */
async function layer(values, spec) {
	const entries = await placements(spec, values.platform);
	await parents([values.metadata, values.compact]);
	const args = ["layer", "--format", "gzip", "--compression-level", "1", "--compressor-jobs", "1",
		"--create-parent-directories", "--default-metadata", JSON.stringify({ mode: "0644", uid: 0, gid: 0, uname: "", gname: "", mtime: epoch }),
		"--metadata", values.metadata, "--compact-stream", values.compact, "--compact-stream-only"];
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-layer-"));
	try {
		// Upstream's tree reader dereferences links. Enumerate leaves ourselves and
		// use its directory entry support only for empty, normalized scaffolding.
		const empty = join(scratch, "empty");
		await mkdir(empty);
		const files = [];
		for (const { path, source, kind, mode } of entries) {
			if (kind === "directories") files.push(path.slice(1) + "\0d" + empty);
			else if (kind === "symlinks") args.push("--symlink", path + "=" + source);
			else {
				files.push(path.slice(1) + "\0f" + source);
				if (kind === "executables" || mode === "0755") args.push("--file-metadata", path.slice(1) + '={"mode":"0755"}');
			}
		}
		if (files.length) {
			const params = join(scratch, "files");
			await writeFile(params, files.join("\n") + "\n");
			args.push("--add-from-file", params);
		}
		await img(values.img, args);
		await layerMetadata(values.metadata);
	} finally {
		await rm(scratch, { recursive: true });
	}
}

/** Reconstruct through a temporary symlink CAS over declared regular payload leaves. */
async function materialize(values, spec) {
	const entries = await placements(spec, values.platform);
	const metadata = await layerMetadata(values.metadata);
	await parents([values.blob]);
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-materialize-"));
	try {
		const inputs = join(scratch, "inputs");
		await writeFile(inputs, entries.filter(({ kind }) => kind === "files" || kind === "executables")
			.map(({ source }) => resolve(source)).join("\n") + "\n");
		const cas = join(scratch, "cas");
		await img(values.img, ["cas-dir", "--symlink", "--output", cas, "--from-file", inputs]);
		const reconstructed = join(scratch, "layer.tgz");
		await img(values.img, ["compact-stream", "reconstruct", "--compact-stream", values.compact, "--cas-dir", cas, "--output", reconstructed]);
		await verifyLayer(reconstructed, metadata, metadata.diff_id);
		await copyFile(reconstructed, values.blob, constants.COPYFILE_EXCL);
	} finally {
		await rm(scratch, { recursive: true });
	}
}

/** Validate and hash an existing tar layer without rewriting its filesystem metadata. */
async function layerFromTar(values) {
	await regularFile(values.archive, false, values.platform);
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-archive-"));
	try {
		await img(values.img, ["mtree", "--tar", values.archive, "--options", "type", "--output", join(scratch, "archive.mtree")]);
		await parents([values.metadata]);
		await img(values.img, ["hash", "--encoding", "layer-metadata", values.archive, values.metadata]);
		const metadata = await layerMetadata(values.metadata);
		await verifyLayer(values.archive, metadata, metadata.diff_id);
	} finally {
		await rm(scratch, { recursive: true });
	}
}

/** Validate config list values, reserving the encoder's inheritance marker. */
function configList(value, name) {
	if (value === null) return [inherit];
	if (!Array.isArray(value)) throw new OciError("OCI_INVALID_SPEC", name + " must be a string list or null");
	for (const item of value) {
		string(item, name, true);
		if (item === inherit) throw new OciError("OCI_RESERVED_VALUE", name + " contains the encoder's inheritance marker");
	}
	return value;
}

/** Compose all base/new layer metadata explicitly, preserving Docker config semantics. */
async function image(values, spec) {
	object(spec, ["base_manifest", "base_config", "base_descriptor", "layers", "entrypoint", "cmd", "env", "labels", "user", "working_dir"], "image specification");
	const arch = architecture(values.platform);
	if (!Array.isArray(spec.layers)) throw new OciError("OCI_INVALID_SPEC", "layers must be metadata paths");
	const baseFields = [spec.base_manifest, spec.base_config, spec.base_descriptor];
	if (baseFields.some((value) => value === null) && !baseFields.every((value) => value === null)) {
		throw new OciError("OCI_INVALID_SPEC", "base metadata must be all present or all absent");
	}
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-image-"));
	try {
		await parents([values.manifest, values.config, values.descriptor]);
		const args = ["manifest", "--os", "linux", "--architecture", arch, "--created-timestamp", epoch,
			"--manifest", values.manifest, "--config", values.config, "--descriptor", values.descriptor];
		// img v0.3.22 declares --base-manifest but does not consume it. Its rootfs
		// is built solely from --layer-from-metadata, including the base layers.
		// https://github.com/bazel-contrib/rules_img/blob/v0.3.22/img_tool/cmd/manifest/manifest.go
		if (spec.base_manifest !== null) {
			const base = await imageMetadata({ platform: values.platform, manifest: spec.base_manifest,
				config: spec.base_config, descriptor: spec.base_descriptor });
			const history = base.config.history ?? base.manifest.layers.map(() => ({ created_by: "history missing" }));
			if (!Array.isArray(history) || history.some((entry) => entry === null || typeof entry !== "object" || Array.isArray(entry))
				|| history.filter((entry) => entry.empty_layer !== true).length !== base.manifest.layers.length) {
				throw new OciError("OCI_INVALID_HISTORY", "base history must describe every nonempty base layer exactly once");
			}
			if (base.manifest.layers.length === 0 && history.length !== 0) {
				throw new OciError("OCI_UNSUPPORTED_BASE_HISTORY", "history-only scratch bases are not qualified by this encoder");
			}
			args.push("--base-config", spec.base_config, "--base-descriptor", spec.base_descriptor, "--stop-signal=" + inherit);
			for (let i = 0; i < base.manifest.layers.length; i++) {
				const path = join(scratch, "base-" + i + ".json");
				const descriptor = base.manifest.layers[i];
				await writeFile(path, JSON.stringify({ mediaType: descriptor.mediaType, digest: descriptor.digest,
					size: descriptor.size, annotations: descriptor.annotations, diff_id: base.config.rootfs.diff_ids[i],
					// The encoder also omits base-config history. Attach it once so its
					// original empty/nonempty entry order precedes every new layer.
					history: i === 0 ? history : [] }));
				args.push("--layer-from-metadata", path);
			}
		}
		for (const path of spec.layers) {
			string(path, "layer metadata path");
			await layerMetadata(path);
			args.push("--layer-from-metadata", path);
		}
		for (const field of ["entrypoint", "cmd"]) {
			for (const item of configList(spec[field], field)) args.push("--" + field + "=" + item);
		}
		for (const [key, flag] of [["user", "user"], ["working_dir", "working-dir"]]) {
			const value = spec[key];
			if (value !== null) {
				string(value, key, true);
				if (value === inherit) throw new OciError("OCI_RESERVED_VALUE", key + " contains the encoder's inheritance marker");
			}
			args.push("--" + flag + "=" + (value === null ? inherit : value));
		}
		if (![null, "", "/"].includes(spec.working_dir)) imagePath(spec.working_dir);
		for (const [field, flag] of [["env", "env"], ["labels", "label"]]) {
			const entries = stringMap(spec[field], field);
			for (const key of Object.keys(entries).sort()) {
				if (key.includes("=")) throw new OciError("OCI_INVALID_SPEC", field + " key must not contain =");
				args.push("--" + flag + "=" + key + "=" + entries[key]);
			}
		}
		await img(values.img, args);
		await imageMetadata({ platform: values.platform, manifest: values.manifest, config: values.config, descriptor: values.descriptor });
	} finally {
		await rm(scratch, { recursive: true });
	}
}

/** Dispatch one recipe through a strict, nonduplicated command argument schema. */
async function main() {
	const [command, ...args] = process.argv.slice(2);
	const fields = {
		layer: ["img", "platform", "spec", "metadata", "compact"],
		materialize: ["img", "platform", "spec", "metadata", "compact", "blob"],
		"layer-from-tar": ["img", "platform", "archive", "metadata"],
		image: ["img", "platform", "spec", "manifest", "config", "descriptor"],
		layout: ["img", "platform", "spec", "output"],
		import: ["platform", "layout", "manifest", "config", "descriptor"],
	}[command];
	if (!fields) throw new OciError("OCI_INVALID_COMMAND", "unknown OCI operation");
	const { values, tokens } = parseArgs({ args, tokens: true, options: Object.fromEntries(fields.map((key) => [key, { type: "string" }])) });
	if (tokens.length !== fields.length) throw new OciError("OCI_INVALID_ARGUMENTS", "each OCI option must be specified exactly once");
	for (const key of fields) string(values[key], key);
	architecture(values.platform);
	const spec = values.spec ? JSON.parse(await readFile(values.spec, "utf8")) : undefined;
	if (command === "layer") await layer(values, spec);
	else if (command === "materialize") await materialize(values, spec);
	else if (command === "layer-from-tar") await layerFromTar(values);
	else if (command === "image") await image(values, spec);
	else if (command === "layout") await exportLayout({ platform: values.platform, spec, output: values.output,
		materialize: async (payload, blob) => materialize({ ...values, metadata: payload.metadata, compact: payload.compact, blob },
			JSON.parse(await readFile(payload.inputs, "utf8"))) });
	else await importLayout(values);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
	main().catch((error) => {
		process.stderr.write((error.code ?? error.name) + ": " + error.message + "\n");
		process.exitCode = 1;
	});
}

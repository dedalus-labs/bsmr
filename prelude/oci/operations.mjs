//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Adapts declared native artifacts to pinned img operations and verified layouts.

import { execFile } from "node:child_process";
import { constants } from "node:fs";
import { chmod, copyFile, lstat, lutimes, mkdir, open, readFile, readdir, readlink, realpath, symlink, utimes } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs, promisify } from "node:util";
import { exportLayout, importLayout, layerMetadata } from "./closure.mjs";

const execute = promisify(execFile);
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

/** Validate complete placements once for both archives and independent contexts. */
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
	for (const entry of entries) {
		if (entry.kind !== "symlinks") await regularFile(entry.source, entry.kind === "executables", platform);
	}
	return entries;
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

/** Encode explicit regular files and symlinks as one ordinary deterministic layer. */
async function layer(values, spec) {
	const entries = await placements(spec, values.platform);
	await parents([values.metadata, values.blob]);
	const args = ["layer", "--format", "gzip", "--compression-level", "1", "--compressor-jobs", "1",
		"--create-parent-directories", "--default-metadata", JSON.stringify({ uid: 0, gid: 0, uname: "", gname: "", mtime: epoch }),
		"--metadata", values.metadata];
	for (const { path, source, kind } of entries) {
		if (kind === "symlinks") args.push("--symlink", path + "=" + source);
		else args.push("--add", path + "=" + source, "--file-metadata",
			path.slice(1) + "=" + JSON.stringify({ mode: kind === "executables" ? "0755" : "0644" }));
	}
	args.push(values.blob);
	await img(values.img, args);
	await layerMetadata(values.metadata);
}

/** Materialize explicit placements without sharing writable payload inodes. */
async function context(values, spec) {
	const entries = await placements(spec, values.platform);
	const output = resolve(values.output);
	await parents([output]);
	await mkdir(output);
	const directories = new Set([output]);
	for (const { path, source, kind } of entries) {
		const destination = join(output, path.slice(1));
		await mkdir(dirname(destination), { recursive: true });
		for (let dir = dirname(destination); dir.startsWith(output); dir = dirname(dir)) {
			directories.add(dir);
			if (dir === output) break;
		}
		if (kind === "symlinks") {
			await symlink(source, destination);
			await lutimes(destination, 0, 0);
		} else {
			await copyFile(source, destination, constants.COPYFILE_EXCL);
			await chmod(destination, kind === "executables" ? 0o755 : 0o644);
			await utimes(destination, 0, 0);
		}
	}
	for (const dir of directories) {
		await chmod(dir, 0o755);
		await utimes(dir, 0, 0);
	}
}

/** Copy and normalize a closed context; links may resolve only within that tree. */
export async function stageContext(source, destination, timestamp) {
	if (!Number.isSafeInteger(timestamp) || timestamp < 0) throw new OciError("OCI_INVALID_TIMESTAMP", "context timestamp must be nonnegative");
	const root = await realpath(source);
	if (!(await lstat(root)).isDirectory()) throw new OciError("OCI_INVALID_CONTEXT", "context must be a directory");
	const destinationRoot = join(await realpath(dirname(destination)), basename(destination));
	const destinationOffset = relative(root, destinationRoot);
	if (destinationOffset === "" || (!destinationOffset.startsWith(".." + sep) && destinationOffset !== ".." && !isAbsolute(destinationOffset))) {
		throw new OciError("OCI_CONTEXT_OVERLAP", "context staging destination must be outside its source tree");
	}
	await mkdir(destination);
	/** Walk declared inputs without dereferencing links into the host filesystem. */
	async function copy(from, to) {
		for (const name of (await readdir(from)).sort()) {
			const input = join(from, name);
			const output = join(to, name);
			const info = await lstat(input);
			if (info.isDirectory()) {
				await mkdir(output);
				await copy(input, output);
			} else if (info.isFile()) {
				await copyFile(input, output, constants.COPYFILE_EXCL);
				await chmod(output, info.mode & 0o111 ? 0o755 : 0o644);
				await utimes(output, timestamp, timestamp);
			} else if (info.isSymbolicLink()) {
				const target = await readlink(input);
				const lexical = relative(root, resolve(dirname(input), target));
				const offset = relative(root, await realpath(input));
				if (isAbsolute(target) || [lexical, offset].some((path) => path === ".." || path.startsWith(".." + sep) || isAbsolute(path))) {
					throw new OciError("OCI_EXTERNAL_CONTEXT_LINK", "context symlink escapes declared inputs: " + input);
				}
				await symlink(target, output);
				await lutimes(output, timestamp, timestamp);
			} else throw new OciError("OCI_NONREGULAR_INPUT", "context contains a special file: " + input);
		}
		await chmod(to, 0o755);
		await utimes(to, timestamp, timestamp);
	}
	await copy(root, destination);
}

/** Dispatch one recipe through a strict, nonduplicated command argument schema. */
async function main() {
	const [command, ...args] = process.argv.slice(2);
	const fields = {
		layer: ["img", "platform", "spec", "metadata", "blob"],
		context: ["platform", "spec", "output"],
		layout: ["platform", "spec", "output"],
		import: ["platform", "layout", "manifest", "config", "descriptor"],
	}[command];
	if (!fields) throw new OciError("OCI_INVALID_COMMAND", "unknown OCI operation");
	const { values, tokens } = parseArgs({ args, tokens: true, options: Object.fromEntries(fields.map((key) => [key, { type: "string" }])) });
	if (tokens.length !== fields.length) throw new OciError("OCI_INVALID_ARGUMENTS", "each OCI option must be specified exactly once");
	for (const key of fields) string(values[key], key);
	architecture(values.platform);
	const spec = values.spec ? JSON.parse(await readFile(values.spec, "utf8")) : undefined;
	if (command === "layer") await layer(values, spec);
	else if (command === "context") await context(values, spec);
	else if (command === "layout") await exportLayout({ platform: values.platform, spec, output: values.output });
	else await importLayout(values);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
	main().catch((error) => {
		process.stderr.write((error.code ?? error.name) + ": " + error.message + "\n");
		process.exitCode = 1;
	});
}

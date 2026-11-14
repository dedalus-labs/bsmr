//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Executes a declared command in an independent Linux rootfs and commits its OCI delta.

import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, lstat, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs, promisify } from "node:util";
import { exportLayout, importLayout } from "./closure.mjs";

const execute = promisify(execFile);
const capabilities = ["CAP_CHOWN", "CAP_DAC_OVERRIDE", "CAP_FOWNER", "CAP_FSETID",
	"CAP_SETGID", "CAP_SETUID", "CAP_SETFCAP", "CAP_KILL"];

/** Report the precise unsupported boundary without selecting another runtime. */
export class OciRunError extends Error {
	constructor(code, message, options) {
		super(message, options);
		this.code = code;
	}
}

/** Require process-safe text; argv may contain whitespace and newlines. */
function text(value, name, empty = false) {
	if (typeof value !== "string" || (!empty && !value) || value.includes("\0")) {
		throw new OciRunError("OCI_RUN_INVALID_SPEC", `${name} must be a string without NUL`);
	}
	return value;
}

/** Validate one normalized path relative to the reserved input directory. */
function inputPath(value) {
	text(value, "input name");
	if (value.includes("\\") || value.split("/").some((part) => ["", ".", ".."].includes(part))) {
		throw new OciRunError("OCI_RUN_INVALID_INPUT_PATH", `input name must be normalized and relative: ${value}`);
	}
}

/** Reject ambiguous recipes before starting tools or creating an output. */
export function validateRunSpec(spec) {
	const fields = ["layout", "platform", "inputs", "command", "env", "user", "working_dir"];
	if (!spec || typeof spec !== "object" || Array.isArray(spec)
		|| Object.keys(spec).length !== fields.length || fields.some((key) => !Object.hasOwn(spec, key))) {
		throw new OciRunError("OCI_RUN_INVALID_SPEC", "run recipe has an unsupported schema");
	}
	text(spec.layout, "base layout");
	if (!["linux/amd64", "linux/arm64"].includes(spec.platform)) {
		throw new OciRunError("OCI_RUN_PLATFORM", "run platform must be linux/amd64 or linux/arm64");
	}
	if (!Array.isArray(spec.command) || spec.command.length === 0) {
		throw new OciRunError("OCI_RUN_INVALID_SPEC", "command requires a nonempty argv");
	}
	spec.command.forEach((value, i) => text(value, `command[${i}]`, i !== 0));
	for (const name of ["inputs", "env"]) {
		if (!spec[name] || typeof spec[name] !== "object" || Array.isArray(spec[name])) {
			throw new OciRunError("OCI_RUN_INVALID_SPEC", `${name} requires a string map`);
		}
		for (const [key, value] of Object.entries(spec[name])) {
			text(key, `${name} key`);
			text(value, `${name} value`, name === "env");
			if (name === "inputs") inputPath(key);
			else if (key.includes("=")) throw new OciRunError("OCI_RUN_INVALID_SPEC", "environment key contains =");
		}
	}
	for (const name of Object.keys(spec.inputs)) {
		for (let parent = dirname(name); parent !== "."; parent = dirname(parent)) {
			if (Object.hasOwn(spec.inputs, parent)) throw new OciRunError("OCI_RUN_INPUT_CONFLICT", `input ancestor overlaps: ${parent}`);
		}
	}
	if (spec.user !== null && !/^(0|[1-9][0-9]*):(0|[1-9][0-9]*)$/u.test(text(spec.user, "user"))) {
		throw new OciRunError("OCI_RUN_USER", "explicit run user must be numeric uid:gid");
	}
	if (spec.user !== null && spec.user.split(":").some((id) => Number(id) > 4294967294)) {
		throw new OciRunError("OCI_RUN_USER", "run uid and gid must fit Linux IDs");
	}
	if (spec.working_dir !== null) {
		text(spec.working_dir, "working_dir");
		if (spec.working_dir !== "/") inputPath(spec.working_dir.slice(1));
		if (!spec.working_dir.startsWith("/")) throw new OciRunError("OCI_RUN_INVALID_SPEC", "working_dir must be absolute");
	}
	return spec;
}

/** Bound every external invocation and propagate cancellation to the owned process. */
async function tool(binary, args, signal) {
	try {
		return await execute(resolve(binary), args, { signal, timeout: 300_000, killSignal: "SIGKILL",
			maxBuffer: 16 * 1024 * 1024, env: { LANG: "C", TZ: "UTC" } });
	} catch (cause) {
		if (cause.stderr) process.stderr.write(cause.stderr);
		throw new OciRunError(signal?.aborted ? "OCI_RUN_CANCELLED" : "OCI_RUN_TOOL_FAILED",
			`${binary} failed${cause.code === undefined ? "" : ` (${cause.code})`}`, { cause });
	}
}

/** Name metadata outputs inside the private work directory. */
function metadata(root) {
	return { manifest: join(root, "manifest.json"), config: join(root, "config.json"), descriptor: join(root, "descriptor.json") };
}

/** Preserve generated runtime defaults while replacing only the declared process. */
export function processConfig(config, spec, inputs) {
	config.process.terminal = false;
	config.process.args = spec.command;
	config.process.noNewPrivileges = true;
	const env = new Map((config.process.env ?? []).map((entry) => {
		const separator = entry.indexOf("=");
		return [entry.slice(0, separator), entry.slice(separator + 1)];
	}));
	for (const entry of Object.entries(spec.env)) env.set(...entry);
	config.process.env = [...env].map(([key, value]) => `${key}=${value}`);
	if (spec.user !== null) {
		const [uid, gid] = spec.user.split(":").map(Number);
		config.process.user = { uid, gid };
	}
	if (spec.working_dir !== null) config.process.cwd = spec.working_dir;
	const allowed = config.process.user.uid === 0 ? capabilities : [];
	config.process.capabilities = { bounding: allowed, effective: allowed, permitted: allowed,
		inheritable: [], ambient: [] };
	config.root.readonly = false;
	config.hostname = "bsmr-oci-run";
	// A fresh network namespace has no uplink. No caller-supplied host mounts are accepted.
	for (const type of ["mount", "pid", "network", "ipc", "uts", "cgroup"]) {
		if (!config.linux.namespaces.some((ns) => ns.type === type && ns.path === undefined)) {
			throw new OciRunError("OCI_RUN_ISOLATION", `umoci bundle lacks a private ${type} namespace`);
		}
	}
	config.mounts.push({ destination: "/inputs", type: "bind", source: inputs, options: ["bind", "ro", "nosuid", "nodev"] });
	return config;
}

/** Copy declared inputs into a private directory so the runtime never mounts source artifacts. */
async function stageInputs(spec, work, bundle) {
	const inputs = join(work, "inputs");
	await mkdir(inputs);
	for (const [name, source] of Object.entries(spec.inputs)) {
		const destination = join(inputs, name);
		await mkdir(dirname(destination), { recursive: true });
		await cp(resolve(source), destination, { recursive: true, dereference: false, verbatimSymlinks: true, errorOnExist: true, force: false });
	}
	// The reserved mountpoint cannot hide an existing image entry or follow a base symlink.
	try { await mkdir(join(bundle, "rootfs/inputs")); } catch (cause) {
		throw new OciRunError("OCI_RUN_INPUT_CONFLICT", "base image must leave /inputs absent", { cause });
	}
	return inputs;
}

/** Kill and delete only the container in this action's private runtime state directory. */
async function deleteContainer(runc, state, id) {
	try {
		const listed = await tool(runc, ["--root", state, "list", "--format", "json"]);
		const containers = JSON.parse(listed.stdout) ?? [];
		if (containers.some((container) => container.id === id)) {
			await tool(runc, ["--root", state, "delete", "--force", id]);
		}
	} catch (cause) {
		throw new OciRunError("OCI_RUN_CLEANUP", `runtime cleanup failed; state retained at ${state}`, { cause });
	}
}

/** Preserve fields that umoci's typed JSON serialization would omit or normalize. */
async function preserveConfig(original, packed, paths) {
	const config = { ...original.config, rootfs: packed.config.rootfs, history: packed.config.history };
	if (original.config.history) config.history.splice(0, original.config.history.length, ...original.config.history);
	const configBytes = Buffer.from(JSON.stringify(config));
	const manifest = { ...original.manifest, layers: packed.manifest.layers, config: { ...original.manifest.config,
		digest: `sha256:${createHash("sha256").update(configBytes).digest("hex")}`, size: configBytes.length } };
	delete manifest.config.data;
	delete manifest.config.urls;
	const manifestBytes = Buffer.from(JSON.stringify(manifest));
	const descriptor = { ...original.descriptor, digest: `sha256:${createHash("sha256").update(manifestBytes).digest("hex")}`,
		size: manifestBytes.length, annotations: { ...original.descriptor.annotations, "org.opencontainers.image.ref.name": "run" } };
	delete descriptor.data;
	delete descriptor.urls;
	await Promise.all([writeFile(paths.config, configBytes), writeFile(paths.manifest, manifestBytes),
		writeFile(paths.descriptor, JSON.stringify(descriptor))]);
}

/** Execute one rootful native Linux command and publish only a verified completed layout. */
export async function runImage({ spec, umoci, runc, output, signal }) {
	validateRunSpec(spec);
	if (process.platform !== "linux" || process.getuid() !== 0) {
		throw new OciRunError("OCI_RUN_HOST", "oci_run requires a rootful Linux execution worker");
	}
	if (`linux/${process.arch === "x64" ? "amd64" : process.arch}` !== spec.platform) {
		throw new OciRunError("OCI_RUN_PLATFORM", "oci_run requires native execution of the image architecture");
	}
	for (const [name, binary] of Object.entries({ umoci, runc })) {
		if (!binary || !(await lstat(binary)).isFile()) throw new OciRunError("OCI_RUN_MISSING_TOOL", `${name} requires a declared executable artifact`);
	}
	try {
		await lstat(output);
		throw new OciRunError("OCI_RUN_OUTPUT_EXISTS", "run output must not already exist");
	} catch (error) { if (error.code !== "ENOENT") throw error; }
	await mkdir(dirname(output), { recursive: true });
	const work = await mkdtemp(join(tmpdir(), "bsmr-oci-run-"));
	const id = basename(work);
	const layout = join(work, "layout"), bundle = join(work, "bundle"), state = join(work, "state");
	await mkdir(state);
	let removable = true;
	try {
		const base = metadata(join(work, "base"));
		const original = await importLayout({ platform: spec.platform, layout: spec.layout, ...base });
		await exportLayout({ platform: spec.platform, spec: { ...base, layers: [], base_layouts: [spec.layout] }, output: layout });
		const index = JSON.parse(await readFile(join(layout, "index.json"), "utf8"));
		index.manifests[0].annotations = { ...index.manifests[0].annotations, "org.opencontainers.image.ref.name": "run" };
		await writeFile(join(layout, "index.json"), JSON.stringify(index));
		await tool(umoci, ["unpack", "--image", `${layout}:run`, bundle], signal);
		const inputs = await stageInputs(spec, work, bundle);
		const config = processConfig(JSON.parse(await readFile(join(bundle, "config.json"), "utf8")), spec, inputs);
		await writeFile(join(bundle, "config.json"), JSON.stringify(config));
		removable = false;
		try {
			const result = await tool(runc, ["--root", state, "run", "--keep", "--bundle", bundle, id], signal);
			process.stdout.write(result.stdout);
			process.stderr.write(result.stderr);
		} finally {
			await deleteContainer(runc, state, id);
			removable = true;
		}
		await rm(join(bundle, "rootfs/inputs"), { recursive: true });
		await tool(umoci, ["repack", "--image", `${layout}:run`, "--no-mask-volumes",
			"--history.created", "1970-01-01T00:00:00Z", "--history.created_by", "oci_run", bundle], signal);
		const result = metadata(join(work, "result"));
		const packed = await importLayout({ platform: spec.platform, layout, ...result });
		await preserveConfig(original, packed, result);
		if (signal?.aborted) throw new OciRunError("OCI_RUN_CANCELLED", "oci_run cancelled before publication");
		// Publication uses the destination filesystem, including when TMPDIR is a separate mount.
		const staged = await mkdtemp(join(dirname(output), ".oci-run-"));
		try {
			await exportLayout({ platform: spec.platform, spec: { ...result, layers: [], base_layouts: [layout] }, output: join(staged, "layout") });
			if (signal?.aborted) throw new OciRunError("OCI_RUN_CANCELLED", "oci_run cancelled before publication");
			await rename(join(staged, "layout"), output);
		} finally { await rm(staged, { recursive: true, force: true }); }
	} finally { if (removable) await rm(work, { recursive: true, force: true }); }
}

/** Adapt the rule's artifact-backed command line to one cancellable run. */
async function main() {
	const { values } = parseArgs({ options: { spec: { type: "string" }, umoci: { type: "string" },
		runc: { type: "string" }, output: { type: "string" } } });
	for (const name of ["spec", "umoci", "runc", "output"]) text(values[name], `--${name}`);
	const controller = new AbortController();
	const cancel = () => controller.abort();
	process.on("SIGINT", cancel);
	process.on("SIGTERM", cancel);
	try {
		await runImage({ spec: JSON.parse(await readFile(values.spec, "utf8")), umoci: values.umoci,
			runc: values.runc, output: resolve(values.output), signal: controller.signal });
	} finally {
		process.off("SIGINT", cancel);
		process.off("SIGTERM", cancel);
	}
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
	main().catch((error) => {
		process.stderr.write(`${error.code ?? "OCI_RUN_FAILED"}: ${error.message}\n`);
		process.exitCode = 1;
	});
}

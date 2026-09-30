//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs a closed-input Dockerfile solve in one pinned disposable BuildKit worker.

import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { parseArgs, promisify } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import { stageContext } from "./operations.mjs";

const execute = promisify(execFile);
const socket = "unix:///run/buildkit/buildkitd.sock";
const sourcePolicy = {
	version: 1,
	rules: [
		{ action: "DENY", selector: { identifier: "*" } },
		{ action: "ALLOW", selector: { identifier: "local://*" } },
	],
};

class WorkerError extends Error {
	/** Name the failed worker boundary without exposing the command's arguments. */
	constructor(code, message) {
		super(message);
		this.name = code;
	}
}

/** Reject unknown schema fields and absent required fields before any mutation. */
function object(value, fields, name) {
	if (value === null || typeof value !== "object" || Array.isArray(value)
		|| Object.keys(value).length !== fields.length || fields.some((field) => !Object.hasOwn(value, field))) {
		throw new WorkerError("InvalidWorkerInput", `${name} has an unsupported schema`);
	}
	return value;
}

/** Require one nonempty string without process argument delimiters. */
function text(value, name) {
	if (typeof value !== "string" || value === "" || /[\0\r\n]/u.test(value)) {
		throw new WorkerError("InvalidWorkerInput", `${name} must be a nonempty string without control delimiters`);
	}
	return value;
}

/** Validate the selected Dockerfile solve without interpreting Dockerfile syntax. */
function solveSpec(path) {
	const spec = object(JSON.parse(readFileSync(path, "utf8")),
		["context", "dockerfile", "platform", "build_args", "target", "source_date_epoch"], "solve specification");
	text(spec.context, "context");
	text(spec.dockerfile, "dockerfile");
	if (/[\\:]/u.test(spec.dockerfile) || spec.dockerfile.split("/").some((part) => ["", ".", ".."].includes(part))) {
		throw new WorkerError("InvalidWorkerInput", "Dockerfile must be a normalized context-relative filename");
	}
	if (!["linux/arm64", "linux/amd64"].includes(spec.platform)) {
		throw new WorkerError("UnsupportedPlatform", "the selected OCI platform is unqualified");
	}
	if (!Number.isSafeInteger(spec.source_date_epoch) || spec.source_date_epoch < 0) {
		throw new WorkerError("InvalidWorkerInput", "source_date_epoch must be a nonnegative safe integer");
	}
	if (spec.target !== null) text(spec.target, "target");
	if (spec.build_args === null || typeof spec.build_args !== "object" || Array.isArray(spec.build_args)) {
		throw new WorkerError("InvalidWorkerInput", "build_args must be a string map");
	}
	for (const [key, value] of Object.entries(spec.build_args)) {
		text(key, "build argument name");
		if (key.includes("=") || key === "SOURCE_DATE_EPOCH" || typeof value !== "string" || value.includes("\0")) {
			throw new WorkerError("InvalidWorkerInput", "build_args contains an unsupported name or value");
		}
	}
	return spec;
}

/** Execute the selected Docker client with private configuration and exact host. */
async function main() {
	const fields = ["docker", "daemon-contract", "buildkit-image", "spec", "output"];
	const { values, tokens } = parseArgs({ tokens: true,
		options: Object.fromEntries(fields.map((key) => [key, { type: "string" }])) });
	if (tokens.length !== fields.length || new Set(tokens.map((token) => token.name)).size !== fields.length) {
		throw new WorkerError("InvalidWorkerArguments", "each worker option must be specified exactly once");
	}
	for (const key of fields) text(values[key], key);
	if (!/^[a-z0-9./:_-]+@sha256:[a-f0-9]{64}$/u.test(values["buildkit-image"])) {
		throw new WorkerError("UnpinnedWorker", "BuildKit image must be identified by a lowercase SHA-256 digest");
	}
	const contract = object(JSON.parse(readFileSync(values["daemon-contract"], "utf8")),
		["host", "version", "api_version", "os", "architecture", "kernel_version"], "daemon contract");
	for (const [key, value] of Object.entries(contract)) text(value, key);
	if (!contract.host.startsWith("unix://") || !isAbsolute(contract.host.slice("unix://".length))) {
		throw new WorkerError("UnsupportedDockerHost", "the qualified worker requires an explicit local Unix socket");
	}
	const spec = solveSpec(values.spec);
	if (contract.os !== "linux" || spec.platform !== `linux/${contract.architecture}`) {
		throw new WorkerError("UnsupportedPlatform", "the qualified worker requires a Linux daemon with the selected native architecture");
	}
	const output = resolve(values.output);
	if (existsSync(output)) throw new WorkerError("OutputExists", "worker output directory must be new");
	const scratch = mkdtempSync(join(tmpdir(), "bsmr-oci-worker-"));
	const config = join(scratch, "docker-config");
	mkdirSync(config);
	writeFileSync(join(config, "config.json"), "{}\n");
	const cancelled = new AbortController();
	const cancel = () => cancelled.abort();
	process.on("SIGINT", cancel);
	process.on("SIGTERM", cancel);
	const cidfile = join(scratch, "container.id");
	let container;
	let complete = false;

	/** Run one bounded protocol operation; cleanup remains usable after cancellation. */
	async function docker(args, boundary, cancellable = true) {
		try {
			return await execute(resolve(values.docker), ["--host", contract.host, "--config", config, ...args], {
				env: { PATH: process.env.PATH, LANG: "C", DOCKER_CONFIG: config },
				maxBuffer: 16 * 1024 * 1024,
				timeout: 300_000,
				signal: cancellable ? cancelled.signal : undefined,
			});
		} catch (error) {
			if (error.stderr) process.stderr.write(error.stderr);
			throw new WorkerError(boundary, cancelled.signal.aborted ? "operation cancelled" : "Docker protocol operation failed");
		}
	}

	try {
		const server = JSON.parse((await docker(["version", "--format", "{{json .Server}}"], "DaemonUnavailable")).stdout);
		const kernel = JSON.parse((await docker(["info", "--format", "{{json .KernelVersion}}"], "DaemonUnavailable")).stdout);
		const observed = { version: server.Version, api_version: server.ApiVersion, os: server.Os,
			architecture: server.Arch, kernel_version: kernel };
		for (const [key, value] of Object.entries(observed)) {
			if (value !== contract[key]) throw new WorkerError("WorkerIdentityMismatch", `declared Docker ${key} does not match the selected daemon`);
		}
		const image = JSON.parse((await docker(["image", "inspect", values["buildkit-image"],
			"--format", "{{json .RepoDigests}}"], "MissingWorkerImage")).stdout);
		if (!Array.isArray(image) || !image.includes(values["buildkit-image"])) {
			throw new WorkerError("WorkerIdentityMismatch", "the offline worker image does not contain the declared repository digest");
		}
		const context = join(scratch, "context");
		await stageContext(resolve(spec.context), context, spec.source_date_epoch);
		writeFileSync(join(scratch, "policy.json"), `${JSON.stringify(sourcePolicy)}\n`);
		// Privilege supplies mounts to this local worker. This is not an isolation
		// contract for hostile Dockerfiles. No privileged BuildKit entitlement is
		// granted, and acquisition/network access stay closed for every solve.
		// BuildKit's worker provider accepts host/cni/bridge, not "none". Host
		// here is this network-disabled Docker container. Individual solves use
		// the separate force-network-mode=none provider and no host entitlement.
		// https://github.com/moby/buildkit/blob/v0.32.2/util/network/netproviders/network.go
		await docker(["create", "--cidfile", cidfile, "--pull=never", "--privileged", "--network=none",
			"--hostname=bsmr-oci-worker", "--name", `bsmr-oci-${randomUUID()}`, "--cpus=4", "--memory=2g", "--pids-limit=512",
			"--tmpfs", "/var/lib/buildkit:rw,exec,size=1g", "--tmpfs", "/run/buildkit:rw,size=64m",
			"--entrypoint", "buildkitd", values["buildkit-image"], "--addr", socket,
			"--root", "/var/lib/buildkit", "--oci-worker-net=host", "--oci-worker-snapshotter=native", "--containerd-worker=false"],
		"WorkerCreateFailed", false);
		container = readFileSync(cidfile, "utf8").trim();
		if (!/^[a-f0-9]{64}$/u.test(container)) throw new WorkerError("WorkerCreateFailed", "Docker returned an invalid created container identity");
		cancelled.signal.throwIfAborted();
		await docker(["start", container], "WorkerStartFailed");
		const deadline = Date.now() + 30_000;
		while (true) {
			const state = JSON.parse((await docker(["inspect", "--format", "{{json .State}}", container], "WorkerUnavailable")).stdout);
			if (!state.Running) {
				process.stderr.write((await docker(["logs", container], "WorkerUnavailable")).stderr);
				throw new WorkerError("WorkerUnavailable", "BuildKit exited before accepting a solve");
			}
			try {
				await docker(["exec", container, "buildctl", "--addr", socket, "debug", "info", "--format", "{{json .}}"], "WorkerNotReady");
				break;
			} catch (error) {
				if (cancelled.signal.aborted || Date.now() >= deadline) throw error;
				await delay(100, undefined, { signal: cancelled.signal });
			}
		}
		await docker(["exec", container, "mkdir", "-p", "/bsmr/context"], "WorkerContextFailed");
		await docker(["cp", `${context}/.`, `${container}:/bsmr/context`], "WorkerContextFailed");
		await docker(["cp", join(scratch, "policy.json"), `${container}:/bsmr/policy.json`], "WorkerContextFailed");
		const build = ["exec", container, "buildctl", "--addr", socket, "build", "--frontend", "dockerfile.v0",
			"--local", "context=/bsmr/context", "--local", "dockerfile=/bsmr/context", "--opt", `filename=${spec.dockerfile}`,
			"--opt", `platform=${spec.platform}`, "--opt", "force-network-mode=none", "--source-policy-file", "/bsmr/policy.json",
			"--opt", `build-arg:SOURCE_DATE_EPOCH=${spec.source_date_epoch}`];
		for (const key of Object.keys(spec.build_args).sort()) build.push("--opt", `build-arg:${key}=${spec.build_args[key]}`);
		if (spec.target !== null) build.push("--opt", `target=${spec.target}`);
		build.push("--output", "type=oci,tar=false,dest=/bsmr/output,oci-mediatypes=true,compression=gzip,compression-level=1,rewrite-timestamp=true");
		await docker(build, "BuildkitSolveFailed");
		mkdirSync(dirname(output), { recursive: true });
		mkdirSync(output);
		await docker(["cp", `${container}:/bsmr/output/.`, output], "WorkerExportFailed");
		complete = true;
	} finally {
		try {
			if (existsSync(cidfile)) {
				const created = readFileSync(cidfile, "utf8").trim();
				if (!/^[a-f0-9]{64}$/u.test(created)) throw new WorkerError("WorkerCleanupFailed", "created container identity is invalid");
				try {
					await docker(["rm", "--force", "--volumes", created], "WorkerCleanupFailed", false);
				} catch {
					complete = false;
					throw new WorkerError("WorkerCleanupFailed", `managed container ${created} was not removed`);
				}
			}
		} finally {
			process.removeListener("SIGINT", cancel);
			process.removeListener("SIGTERM", cancel);
			if (!complete && existsSync(output)) rmSync(output, { recursive: true });
			rmSync(scratch, { recursive: true });
		}
	}
}

main().catch((error) => {
	process.stderr.write(`${error.name}: ${error.message}\n`);
	process.exitCode = 1;
});

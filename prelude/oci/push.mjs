//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Publishes prepared OCI bytes with explicit runtime credentials outside the action cache.

import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { registryEnvironment } from "./auth.mjs";
import { executeClient } from "./client.mjs";

/** Publish one prepared image without recording credentials or registry error bodies. */
async function main() {
	const { values } = parseArgs({ options: {
		img: { type: "string" }, request: { type: "string" }, layout: { type: "string" }, sink: { type: "string" },
	} });
	for (const name of ["img", "request", "layout"]) {
		if (!values[name] || /[\0\r\n]/u.test(values[name])) throw new Error(`OCI_PUSH_INVALID_ARGUMENT: --${name} is required`);
	}
	const args = ["deploy", "--request-file", resolve(values.request), "--oci-layout", resolve(values.layout)];
	if (values.sink !== undefined) {
		if (!values.sink.startsWith("oci:") || !isAbsolute(values.sink.slice(4))) {
			throw new Error("OCI_PUSH_INVALID_SINK: testing sink must be oci:/absolute/path");
		}
		args.push("--sink", values.sink);
	}
	const request = JSON.parse(await readFile(values.request, "utf8"));
	if (!Array.isArray(request.operations) || request.operations.length !== 1 || request.operations[0].command !== "push") {
		throw new Error("OCI_PUSH_INVALID_REQUEST: publication requires exactly one push operation");
	}
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-push-auth-"));
	const controller = new AbortController();
	const cancel = () => controller.abort();
	process.on("SIGINT", cancel);
	process.on("SIGTERM", cancel);
	try {
		const hasAuth = Object.keys(process.env).some((name) => name.startsWith("IMG_REGISTRY_AUTH_"));
		const env = registryEnvironment(request.operations[0].registry, scratch, hasAuth ? process.env : null);
		await writeFile(join(scratch, "config.json"), "{}", { mode: 0o600, flag: "wx" });
		let result;
		try {
			result = await executeClient(resolve(values.img), args, { cwd: scratch, env, signal: controller.signal,
				timeout: 300_000, killSignal: "SIGKILL", maxBuffer: 16 * 1024 * 1024 });
		} catch {
			// A registry may echo request headers in an error body. Never print those bytes.
			throw new Error(controller.signal.aborted ? "OCI_PUSH_CANCELLED: publication was interrupted"
				: "OCI_PUSH_FAILED: pinned img could not publish the prepared image; check registry access and TLS trust");
		}
		process.stdout.write(result.stdout);
		process.stderr.write(result.stderr);
	} finally {
		process.removeListener("SIGINT", cancel);
		process.removeListener("SIGTERM", cancel);
		await rm(scratch, { recursive: true });
	}
}

main().catch((error) => {
	process.stderr.write((error.code ? error.code + ": " : "") + error.message + "\n");
	process.exitCode = 1;
});

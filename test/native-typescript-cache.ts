//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native TypeScript cache restoration, invalidation, and output isolation.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

const binary = process.argv[2];
assert.ok(binary, "pass the BSMR binary under test");
const executable = resolve(binary);
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-typescript-cache-")));
const cwd = join(root, "workspace");
const options = { cwd, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") }, timeout: 180_000 };
cpSync(fileURLToPath(new URL("./fixtures/typescript-cache", import.meta.url)), cwd, { recursive: true });

type Action = { identity: string; reproducer: { executor: string } };
type Executors = { pnpm_install: "Local" | "Cache"; typescript_library: "Local" | "Cache" };

/** Executes the built Hollywood action outside the checkout and installed dependencies. */
async function verifyAction(output: string, value: string) {
	const runtime = join(root, "action runtime");
	rmSync(runtime, { recursive: true, force: true });
	mkdirSync(runtime);
	cpSync(output, join(runtime, "bundle"), { recursive: true, dereference: true });
	writeFileSync(join(runtime, "input file.txt"), "payload\n");
	const outputs = join(runtime, "outputs");
	writeFileSync(outputs, "");
	const actionOptions = {
		cwd: runtime,
		env: { INPUT_SOURCE: "input file.txt", GITHUB_OUTPUT: outputs },
		timeout: 10_000,
	};
	const entrypoint = join(runtime, "bundle/action.mjs");
	assert.equal(realpathSync(entrypoint), entrypoint, "action must execute the detached copy");
	await run(process.execPath, [entrypoint], actionOptions);
	const record = /^value<<([^\r\n]+)\r?\n([^\r\n]+)\r?\n\1\r?\n$/.exec(readFileSync(outputs, "utf8"));
	assert.equal(record?.[2], `${value}:payload`, "bundled action must write its declared output");
	writeFileSync(outputs, "");
	rmSync(join(runtime, "input file.txt"));
	await assert.rejects(run(process.execPath, [entrypoint], actionOptions), {
		code: 1,
		stdout: /ENOENT.*input file\.txt/,
	});
	assert.equal(readFileSync(outputs, "utf8"), "", "failed action must not publish an output");
	return createHash("sha256").update(readFileSync(entrypoint)).digest("hex");
}

/** Builds a real module and verifies the executors from this exact invocation. */
async function build(expected: Executors, value: string) {
	const { stdout, stderr } = await run(executable, ["build", "probe", "--show-full-json-output", "--console", "simple"], options);
	const outputs: Record<string, string> = JSON.parse(stdout);
	assert.equal(Object.keys(outputs).length, 1);
	const output = Object.values(outputs)[0];
	assert.ok(output);
	const program = join(output, "index.mjs");
	const moduleUrl = JSON.stringify(pathToFileURL(program).href);
	assert.equal((await run(process.execPath, ["--input-type=module", "-e", `import { value } from ${moduleUrl}; process.stdout.write(value);`])).stdout, value);
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(executable, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "pnpm_install|typescript_library", "--no-remote"], options);
	const actions: Action[] = log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	assert.equal(actions.length, 2);
	const executors = Object.fromEntries(actions.map(({ identity, reproducer }) => [
		/\((pnpm_install|typescript_library) /.exec(identity)?.[1], reproducer.executor,
	]));
	assert.deepEqual(executors, expected);
	const digest = createHash("sha256").update(readFileSync(program)).digest("hex");
	const actionDigest = await verifyAction(output, value);
	process.stdout.write(`${JSON.stringify({ trace, executors, digest, actionDigest })}\n`);
	return { program, digest, actionDigest };
}

try {
	await run(executable, ["init"], options);
	if (process.argv[3] !== undefined) {
		cpSync(resolve(process.argv[3]), join(cwd, "prelude"), { recursive: true });
		writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	const first = await build({ pnpm_install: "Local", typescript_library: "Local" }, "original");
	const installed = await run(executable, ["build", ":__bsmr_dependencies", "--show-full-json-output", "--console", "simple"], options);
	const installPaths = Object.values(JSON.parse(installed.stdout) as Record<string, string>);
	assert.equal(installPaths.length, 1);
	const installPath = installPaths[0];
	assert.ok(installPath);
	assert.equal(realpathSync(installPath), installPath, "publish the installed workspace in place without relocating its tree");
	await run(executable, ["clean"], options);
	assert.equal(existsSync(first.program), false, "clean must remove the compiled output");
	const restored = await build({ pnpm_install: "Cache", typescript_library: "Cache" }, "original");
	assert.equal(restored.digest, first.digest);
	assert.equal(restored.actionDigest, first.actionDigest);
	writeFileSync(restored.program, "mutated output");
	await run(executable, ["clean"], options);
	const isolated = await build({ pnpm_install: "Cache", typescript_library: "Cache" }, "original");
	assert.equal(isolated.digest, first.digest, "writable output must not change cached bytes");
	writeFileSync(join(cwd, "probe/src/index.ts"), 'export const value: string = "changed";\n');
	await run(executable, ["clean"], options);
	const changed = await build({ pnpm_install: "Cache", typescript_library: "Local" }, "changed");
	assert.notEqual(changed.digest, first.digest);
	assert.notEqual(changed.actionDigest, first.actionDigest);
	process.stdout.write("ok: TypeScript cache and standalone Hollywood action outputs\n");
} finally {
	await run(executable, ["kill"], options);
	rmSync(root, { recursive: true });
}

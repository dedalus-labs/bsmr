//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native TypeScript cache restoration, invalidation, and output isolation.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
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
	process.stdout.write(`${JSON.stringify({ trace, executors, digest })}\n`);
	return { program, digest };
}

try {
	await run(executable, ["init"], options);
	const first = await build({ pnpm_install: "Local", typescript_library: "Local" }, "original");
	await run(executable, ["clean"], options);
	assert.equal(existsSync(first.program), false, "clean must remove the compiled output");
	const restored = await build({ pnpm_install: "Cache", typescript_library: "Cache" }, "original");
	assert.equal(restored.digest, first.digest);
	writeFileSync(restored.program, "mutated output");
	await run(executable, ["clean"], options);
	const isolated = await build({ pnpm_install: "Cache", typescript_library: "Cache" }, "original");
	assert.equal(isolated.digest, first.digest, "writable output must not change cached bytes");
	writeFileSync(join(cwd, "probe/src/index.ts"), 'export const value: string = "changed";\n');
	await run(executable, ["clean"], options);
	const changed = await build({ pnpm_install: "Cache", typescript_library: "Local" }, "changed");
	assert.notEqual(changed.digest, first.digest);
	process.stdout.write("ok: TypeScript compilation, restoration, output isolation, source invalidation\n");
} finally {
	await run(executable, ["kill"], options);
	rmSync(root, { recursive: true });
}

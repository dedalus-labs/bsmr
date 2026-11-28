//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies that native TypeScript packages run their vitest suites through bsmr test.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { appendFileSync, cpSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const binary = process.argv[2];
assert.ok(binary, "pass the BSMR binary under test");
const executable = resolve(binary);
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-typescript-test-")));
const cwd = join(root, "workspace");
const options = { cwd, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") }, timeout: 300_000 };
cpSync(fileURLToPath(new URL("../fixtures/typescript-test", import.meta.url)), cwd, { recursive: true });

type Outcome = Readonly<{ code: number; output: string }>;

/**
 * Run bsmr test on one selector and capture its exit code and combined output.
 *
 * @param target - Package path or label to test.
 * @param executorArguments - Words after `--` for the test executor.
 * @returns The command's exit code and output.
 */
async function test(target: string, ...executorArguments: readonly string[]): Promise<Outcome> {
	const started = performance.now();
	try {
		const { stdout, stderr } = await run(executable, ["test", target, "--console", "simple", "--", ...executorArguments], options);
		return { code: 0, output: stdout + stderr };
	} catch (error) {
		const failure = error as { code?: unknown; stdout?: string; stderr?: string };
		if (typeof failure.code !== "number") throw error;
		return { code: failure.code, output: (failure.stdout ?? "") + (failure.stderr ?? "") };
	} finally {
		process.stdout.write(JSON.stringify({ target, ms: Math.round(performance.now() - started) }) + "\n");
	}
}

try {
	await run(executable, ["init"], options);
	if (process.argv[3] !== undefined) {
		cpSync(resolve(process.argv[3]), join(cwd, "prelude"), { recursive: true });
		appendFileSync(join(cwd, ".bsmr"), "\n[external_cells]\nprelude = disabled\n");
	}

	const failing = await test("failing");
	assert.notEqual(failing.code, 0, "a failing vitest test must fail the command:\n" + failing.output);
	assert.match(failing.output, /Tests\s+1 failed \| 1 passed \(2\)/, "both tests must run:\n" + failing.output);

	const filtered = await test("failing", "--test-arg", "--testNamePattern", "adds");
	assert.equal(filtered.code, 0, "forwarded vitest arguments must reach vitest:\n" + filtered.output);
	assert.match(filtered.output, /Tests\s+1 passed \| 1 skipped \(2\)/, filtered.output);

	const passing = await test("passing");
	assert.equal(passing.code, 0, passing.output);
	assert.match(passing.output, /Pass: root\/\/passing:test/, passing.output);

	const empty = await test("empty");
	assert.notEqual(empty.code, 0, "a declared test runner without tests must fail:\n" + empty.output);
	assert.match(empty.output, /No test files found/, empty.output);

	const plain = await test("plain");
	assert.equal(plain.code, 0, plain.output);
	assert.match(plain.output, /NO TESTS RAN/, "a package without a test runner has no tests:\n" + plain.output);

	const root = await test("//:");
	assert.equal(root.code, 0, root.output);
	assert.doesNotMatch(root.output, /root\/\/:test\b/, "the workspace root's vitest is shared tooling:\n" + root.output);

	const named = await test("test:test");
	assert.equal(named.code, 0, "a package directory named test keeps its test target:\n" + named.output);
	assert.match(named.output, /Pass: root\/\/test:test/, named.output);
	process.stdout.write("ok: TypeScript tests decide the bsmr test exit code\n");
} finally {
	await run(executable, ["kill"], options);
	rmSync(root, { recursive: true });
}

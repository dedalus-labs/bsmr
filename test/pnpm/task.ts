//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Exercises a package task through BSMR with real locked compilers and output consumers.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

assert.ok(process.argv[2], "pass the BSMR binary under test");
const binary = resolve(process.argv[2]);
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-task-")));
const cwd = join(root, "workspace");
const options = { cwd, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") }, timeout: 180_000 };
const setupStarted = performance.now();
cpSync(fileURLToPath(new URL("../fixtures/typescript-cache", import.meta.url)), cwd, { recursive: true });
const manifest = JSON.parse(readFileSync(join(cwd, "probe/package.json"), "utf8"));
manifest.scripts = { build: "tsdown && node assets.mjs" };
writeFileSync(join(cwd, "probe/package.json"), JSON.stringify(manifest));
writeFileSync(join(cwd, "probe/assets.mjs"), 'import { mkdirSync, writeFileSync } from "node:fs"; mkdirSync("web"); writeFileSync("web/index.html", "complete");');
mkdirSync(join(cwd, "tasks"));
writeFileSync(join(cwd, "tasks/BUILD.bsmr"), `load("@prelude//toolchains/pnpm:defs.bzl", "pnpm_task")
pnpm_task(
    name = "build",
    install = "root//:__bsmr_dependencies",
    package_root = "probe",
    script = "build",
    source = "root//probe:__bsmr_sources",
    outputs = {"dist": "directory", "web/index.html": "file"},
)
`);

process.stdout.write(JSON.stringify({ phase: "fixture", milliseconds: Math.round(performance.now() - setupStarted) }) + "\n");

/** Measure one boundary independently, including failed attempts. */
async function phase<T>(name: string, operation: () => Promise<T>): Promise<T> {
	const started = performance.now();
	let outcome = "failure";
	try {
		const result = await operation();
		outcome = "success";
		return result;
	} finally {
		process.stdout.write(JSON.stringify({ phase: name, outcome, milliseconds: Math.round(performance.now() - started) }) + "\n");
	}
}

/** Build the complete package and consume its output outside the workspace. */
async function build(name: string, executed: boolean): Promise<void> {
	const { stdout, stderr } = await phase(`${name}/build`, () => run(binary, ["build", "tasks:build", "--show-full-json-output", "--console", "simple"], options));
	const paths: string[] = Object.values(JSON.parse(stdout));
	assert.equal(paths.length, 1);
	const output = paths[0];
	assert.ok(output);
	const detached = join(root, "detached");
	await phase(`${name}/copy`, async () => {
		rmSync(detached, { recursive: true, force: true });
		cpSync(output, detached, { recursive: true, dereference: true });
	});
	const cli = join(detached, "dist/index.mjs");
	const { stdout: value } = await phase(`${name}/execute`, () => run(process.execPath, ["--input-type=module", "-e", `import { value } from ${JSON.stringify(cli)}; console.log(value);`]));
	assert.equal(value.trim(), "original");
	assert.equal(readFileSync(join(detached, "web/index.html"), "utf8"), "complete");
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace);
	const log = await phase(`${name}/receipt`, () => run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "pnpm_task", "--no-remote"], options));
	const actions = log.stdout.trim() === "" ? [] : log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	assert.deepEqual(actions.map((action) => action.reproducer.executor), executed ? ["Local"] : []);
	const critical = await phase(`${name}/profile`, () => run(binary, ["log", "critical-path", "--trace-id", trace, "--format", "json", "--no-remote"], options));
	for (const line of critical.stdout.trim().split("\n").filter(Boolean)) {
		process.stdout.write(JSON.stringify({ phase: `${name}/critical-path`, trace, entry: JSON.parse(line) }) + "\n");
	}
}

try {
	await phase("init", () => run(binary, ["init"], options));
	const prelude = process.argv[3];
	if (prelude !== undefined) {
		await phase("source-prelude", async () => {
			cpSync(resolve(prelude), join(cwd, "prelude"), { recursive: true });
			writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
		});
	}
	await build("cold", true);
	await build("warm", false);
	const selected = await phase("select", () => run(binary, ["build", "tasks:build[web/index.html]", "--show-full-json-output", "--console", "simple"], options));
	const files: string[] = Object.values(JSON.parse(selected.stdout));
	assert.ok(files[0]);
	assert.equal(readFileSync(files[0], "utf8"), "complete", "consumers can select individual declared artifacts");
	await phase("clean", () => run(binary, ["clean"], options));
	await build("rebuild", true);
	writeFileSync(join(cwd, "probe/assets.mjs"), "// Successful command that forgot its required artifact.\n");
	await phase("missing-output", () => assert.rejects(run(binary, ["build", "tasks:build", "--console", "simple"], options), { stderr: /ENOENT.*web\/index.html/ }));
	process.stdout.write("ok: native package script, complete outputs, warm reuse, rebuild, missing-output rejection\n");
} finally {
	await phase("stop", () => run(binary, ["kill"], options));
	await phase("cleanup", async () => rmSync(root, { recursive: true }));
}

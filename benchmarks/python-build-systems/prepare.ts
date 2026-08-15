//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Creates the exact Django source fixture shared by the BSMR and Bazel runners.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { djangoCommit, djangoRepository } from "./config.ts";

const configuration = "\n[tool.bsmr.python]\ntest-command = [\"benchmark_test.py\"]\n\n[tool.uv]\ncache-keys = [{ file = \"pyproject.toml\" }, { git = { commit = true } }]\n";

/** Executes one fixture-creation command and fails with its complete output. */
const execute = (executable: string, args: readonly string[], cwd: string): string => {
	try {
		return execFileSync(executable, args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }).trim();
	} catch (error) {
		const failure = error as { stderr?: Buffer | string; stdout?: Buffer | string };
		throw new Error(`command failed: ${executable} ${args.join(" ")}\n${String(failure.stdout ?? "")}${String(failure.stderr ?? "")}`);
	}
};

/** Copies the benchmark overlay while preserving Django's upstream source tree. */
export const applyFixture = (repository: string, fixture = join(import.meta.dirname, "fixture")): void => {
	const project = join(repository, "pyproject.toml");
	const pyproject = readFileSync(project, "utf8");
	if (pyproject.includes("[tool.uv]")) throw new Error("Django fixture unexpectedly declares [tool.uv]");
	writeFileSync(project, `${pyproject.trimEnd()}\n${configuration}`);
	for (const name of ["BUILD.bazel", "MODULE.bazel", "benchmark_main.py", "benchmark_test.py", "pylock.build.toml", "pylock.toml", "requirements.txt"]) {
		cpSync(join(fixture, name), join(repository, name));
	}
	cpSync(join(fixture, "pylock.toml"), join(repository, "pylock.test.toml"));
	cpSync(join(fixture, "bsmrconfig"), join(repository, ".bsmr"));
};

/** Creates one immutable, commit-pinned Django benchmark checkout. */
const main = (): void => {
	const destination = resolve(process.env["BSMR_BENCH_REPOSITORY"] ?? "");
	if (!process.env["BSMR_BENCH_REPOSITORY"]) throw new Error("BSMR_BENCH_REPOSITORY must name a new fixture directory");
	if (existsSync(destination)) throw new Error(`benchmark repository already exists: ${destination}`);
	mkdirSync(destination, { recursive: true });
	execute("git", ["init", "--quiet"], destination);
	execute("git", ["remote", "add", "origin", djangoRepository], destination);
	execute("git", ["fetch", "--depth=1", "origin", djangoCommit], destination);
	execute("git", ["checkout", "--quiet", "-b", "bsmr-benchmark", "FETCH_HEAD"], destination);
	const observed = execute("git", ["rev-parse", "HEAD"], destination);
	if (observed !== djangoCommit) throw new Error(`expected Django ${djangoCommit}, fetched ${observed}`);
	applyFixture(destination);
	process.stdout.write(`${destination}\n`);
};

if (import.meta.main) main();

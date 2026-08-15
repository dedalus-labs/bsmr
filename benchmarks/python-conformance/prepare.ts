//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Checks out and locks the immutable Python conformance corpus.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import {
	buildLockArguments,
	pythonCorpus,
	pythonCorpusPythonVersion,
	pythonCorpusUvVersion,
	runtimeExportArguments,
} from "./corpus.ts";

/** Requires one non-empty environment setting. */
const setting = (name: string): string => {
	const value = process.env[name];
	if (!value) throw new Error(`${name} is required`);
	return value;
};

/** Executes one argv-only preparation command and returns trimmed stdout. */
const execute = (executable: string, args: readonly string[], cwd: string, environment = process.env): string => {
	try {
		return execFileSync(executable, args, { cwd, encoding: "utf8", env: environment, maxBuffer: 256 * 1024 * 1024 }).trim();
	} catch (error) {
		const failure = error as { stderr?: Buffer | string; stdout?: Buffer | string };
		throw new Error(`command failed: ${executable} ${args.join(" ")}\n${String(failure.stdout ?? "")}${String(failure.stderr ?? "")}`);
	}
};

/** Rejects a tool whose self-reported identity differs from the corpus pin. */
const requireVersion = (executable: string, expected: string, cwd: string): void => {
	const observed = execute(executable, ["--version"], cwd, { LANG: "C.UTF-8", PATH: "/bin:/usr/bin" });
	if (!observed.split(/\s+/).includes(expected)) throw new Error(`expected ${executable} ${expected}, got '${observed}'`);
};

/** Fetches one exact commit without consulting a mutable default branch. */
const checkout = (destination: string, repository: string, commit: string): void => {
	mkdirSync(destination);
	execute("git", ["init", "--quiet"], destination);
	execute("git", ["remote", "add", "origin", repository], destination);
	execute("git", ["fetch", "--depth=1", "origin", commit], destination);
	execute("git", ["checkout", "--quiet", "-b", "bsmr-benchmark", "FETCH_HEAD"], destination);
	const observed = execute("git", ["rev-parse", "HEAD"], destination);
	if (observed !== commit) throw new Error(`expected ${repository} ${commit}, fetched ${observed}`);
};

/** Materializes every pinned checkout and its standard runtime and build locks. */
const main = (): void => {
	const root = resolve(setting("BSMR_BENCH_CORPUS_ROOT"));
	const uv = resolve(setting("BSMR_BENCH_UV"));
	const python = resolve(setting("BSMR_BENCH_PYTHON"));
	if (existsSync(root)) throw new Error(`benchmark corpus root already exists: ${root}`);
	if (!existsSync(uv)) throw new Error(`BSMR_BENCH_UV does not exist: ${uv}`);
	if (!existsSync(python)) throw new Error(`BSMR_BENCH_PYTHON does not exist: ${python}`);
	mkdirSync(root, { recursive: true });
	requireVersion(uv, pythonCorpusUvVersion, root);
	requireVersion(python, pythonCorpusPythonVersion, root);
	const configuration = readFileSync(join(import.meta.dirname, "fixture", "bsmrconfig"), "utf8");
	const state = join(root, ".state");
	for (const path of [state, join(state, "home"), join(state, "uv-cache"), join(state, "xdg-cache"), join(state, "xdg-config")]) mkdirSync(path, { recursive: true });
	const environment: NodeJS.ProcessEnv = {
		HOME: join(state, "home"),
		LANG: "C.UTF-8",
		PATH: "/bin:/usr/bin",
		UV_CACHE_DIR: join(state, "uv-cache"),
		UV_PYTHON: python,
		UV_PYTHON_DOWNLOADS: "never",
		XDG_CACHE_HOME: join(state, "xdg-cache"),
		XDG_CONFIG_HOME: join(state, "xdg-config"),
	};
	for (const project of pythonCorpus) {
		const destination = join(root, project.name);
		checkout(destination, project.repository, project.commit);
		writeFileSync(join(destination, ".bsmr"), configuration);
		writeFileSync(join(destination, "pylock.build.in"), `${project.buildRequirements.join("\n")}\n`);
		execute(uv, runtimeExportArguments(), destination, environment);
		execute(uv, buildLockArguments(), destination, environment);
	}
	process.stdout.write(`${root}\n`);
};

if (import.meta.main) main();

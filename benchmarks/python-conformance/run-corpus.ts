//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs differential conformance over every pinned real-world Python project.

import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { pythonCorpus } from "./corpus.ts";

/** Requires one non-empty environment setting. */
const setting = (name: string): string => {
	const value = process.env[name];
	if (!value) throw new Error(`${name} is required`);
	return value;
};

/** Runs one corpus member and returns its immutable result document. */
const runProject = (binary: string, root: string, runRoot: string, id: string, project: typeof pythonCorpus[number]): unknown => {
	const repository = join(root, project.name);
	if (!existsSync(repository)) throw new Error(`prepared corpus member does not exist: ${repository}`);
	const output = execFileSync(process.execPath, [join(import.meta.dirname, "run.ts")], {
		cwd: resolve(import.meta.dirname, "..", ".."),
		encoding: "utf8",
		env: {
			...process.env,
			BSMR_BENCH_BINARY: binary,
			BSMR_BENCH_CACHE_STATE: "empty-isolation",
			BSMR_BENCH_ISOLATION_DIR: `python-corpus-${id}-${project.name}`,
			BSMR_BENCH_PYTHON_IMPORTS: project.imports.join(","),
			BSMR_BENCH_PYTHON_PROJECT_ENVIRONMENT: "root//:__bsmr_python_workspace_environment",
			BSMR_BENCH_PYTHON_SOURCE_ROOTS: project.sourceRoots.join(","),
			BSMR_BENCH_REPOSITORY: repository,
			BSMR_BENCH_ROOT: runRoot,
		},
		maxBuffer: 256 * 1024 * 1024,
	}).trim();
	const result = output.split("\n").at(-1);
	if (!result || !existsSync(result)) throw new Error(`conformance runner returned no result for ${project.name}: '${output}'`);
	return JSON.parse(readFileSync(result, "utf8"));
};

/** Produces one aggregate, fail-closed corpus result. */
const main = (): void => {
	const binary = resolve(setting("BSMR_BENCH_BINARY"));
	const root = resolve(setting("BSMR_BENCH_CORPUS_ROOT"));
	if (!existsSync(binary)) throw new Error(`BSMR_BENCH_BINARY does not exist: ${binary}`);
	if (!existsSync(root)) throw new Error(`BSMR_BENCH_CORPUS_ROOT does not exist: ${root}`);
	const id = randomUUID();
	const runRoot = resolve(process.env["BSMR_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `python-corpus-${Date.now()}-${id}`);
	mkdirSync(runRoot, { recursive: true });
	const projects = Object.fromEntries(pythonCorpus.map((project) => [project.name, runProject(binary, root, runRoot, id, project)]));
	const result = join(runRoot, "results.json");
	writeFileSync(result, `${JSON.stringify({ correctness: "pass", projects }, null, 2)}\n`);
	process.stdout.write(`${result}\n`);
};

if (import.meta.main) main();

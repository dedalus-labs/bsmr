//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the immutable real-world repository corpus for Python conformance.

export interface PythonCorpusProject {
	buildRequirements: readonly string[];
	commit: string;
	imports: readonly string[];
	name: string;
	repository: string;
	sourceRoots: readonly string[];
}

export const pythonCorpus = [
	{
		buildRequirements: ["uv_build>=0.8.3,<0.9.0"],
		commit: "d0857364e8a727be41b181731e03f478213e4558",
		imports: ["scripts"],
		name: "cosmos-cookbook",
		repository: "https://github.com/nvidia-cosmos/cosmos-cookbook.git",
		sourceRoots: ["."],
	},
	{
		buildRequirements: ["hatchling==1.26.3", "hatch-fancy-pypi-readme"],
		commit: "1aae8531856530f688426d113e242cc9ff6c50e5",
		imports: ["dedalus_labs"],
		name: "dedalus-agents-python",
		repository: "https://github.com/dedalus-labs/dedalus-agents-python.git",
		sourceRoots: ["."],
	},
	{
		buildRequirements: ["hatchling", "uv-dynamic-versioning>=0.7.0"],
		commit: "25a70926cfafdfc63b3d32c1b5f2c7f139e2c58c",
		imports: ["pydantic_ai", "pydantic_graph", "pydantic_evals"],
		name: "pydantic-ai",
		repository: "https://github.com/pydantic/pydantic-ai.git",
		sourceRoots: [".", "pydantic_ai_slim", "pydantic_graph", "pydantic_evals"],
	},
] as const satisfies readonly PythonCorpusProject[];

export const pythonCorpusCutoff = "2026-08-15T00:00:00Z";
export const pythonCorpusPythonVersion = "3.14.7";
export const pythonCorpusUvVersion = "0.12.5";

/** Returns stable uv arguments for exporting an upstream lock without resolution. */
export const runtimeExportArguments = (): readonly string[] => [
	"export",
	"--frozen",
	"--format",
	"pylock.toml",
	"--no-default-groups",
	"--no-emit-workspace",
	"--output-file",
	"pylock.toml",
];

/** Returns stable uv arguments for one universal PEP 517 build lock. */
export const buildLockArguments = (): readonly string[] => [
	"pip",
	"compile",
	"pylock.build.in",
	"--format",
	"pylock.toml",
	"--universal",
	"--exclude-newer",
	pythonCorpusCutoff,
	"--output-file",
	"pylock.build.toml",
];

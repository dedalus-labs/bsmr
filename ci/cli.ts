//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Routes repository CI commands through Hollywood's typed process executor.

import { realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { nodeExec, type ScriptExec } from "@dedalus-labs/hollywood";

type ProcessSpec = Readonly<{ file: string; args: readonly string[] }>;
export type CliContext = Readonly<{
	root: string;
	exec: ScriptExec;
	stdout: { write(value: string): unknown };
	stderr: { write(value: string): unknown };
}>;

const buildActions: ProcessSpec = {
	file: "pnpm",
	args: ["exec", "rolldown", "--config", "rolldown.config.ts"],
};
const typecheck: ProcessSpec = { file: "pnpm", args: ["exec", "tsc", "--noEmit"] };
const test: ProcessSpec = {
	file: "node",
	args: ["--test", "ci/ci.test.ts", "ci/cli.test.ts", "ci/license-preamble.test.ts", "ci/license-provenance.test.ts", "ci/license.test.ts", "test/contributors.test.ts"],
};
const license: ProcessSpec = { file: "node", args: ["ci/license.ts", "check"] };
const generated: ProcessSpec = {
	file: "pnpm",
	args: ["exec", "hollywood", "check", "--generated", "--source-root", "ci", "--output", "."],
};
const security: ProcessSpec = {
	file: "pnpm",
	args: [
		"exec",
		"hollywood",
		"check",
		"--workflow-security",
		"--source-root",
		".github/workflows",
		"--output",
		".",
	],
};
const actionDiff: ProcessSpec = {
	file: "git",
	args: ["diff", "--exit-code", "--", ".github/actions"],
};
const actionSyntax: ProcessSpec = {
	file: "node",
	args: ["--check", ".github/actions/ci/rust-affected/dist/index.js"],
};
const generate: ProcessSpec = {
	file: "pnpm",
	args: ["exec", "hollywood", "generate", "ci/**/*.ts", "--output", "."],
};

const commands = {
	"build actions": [buildActions],
	"check actions": [buildActions, actionSyntax, actionDiff],
	"check generated": [generated],
	"check license": [license],
	"check security": [security],
	check: [typecheck, test, generated, buildActions, actionSyntax, actionDiff, security],
	generate: [generate, buildActions],
	test: [test],
	typecheck: [typecheck],
} as const satisfies Record<string, readonly ProcessSpec[]>;

const usage = `Usage: pnpm run ci <command>\n\nCommands:\n  build actions\n  check\n  check actions\n  check generated\n  check license\n  check security\n  generate\n  test\n  typecheck\n`;

/**
 * Create the process execution context rooted at the repository.
 *
 * @returns The production CLI context.
 */
function defaultContext(): CliContext {
	return {
		root: resolve(dirname(fileURLToPath(import.meta.url)), ".."),
		exec: nodeExec,
		stdout: process.stdout,
		stderr: process.stderr,
	};
}

/**
 * Run the CI command selected by its command-line words.
 *
 * @param arguments_ - Command words after the CLI entrypoint.
 * @param context - Process execution and output dependencies.
 * @returns A promise that resolves after every command succeeds.
 * @throws An error when the command is unknown or a process fails.
 */
export async function runCli(arguments_: readonly string[], context: CliContext): Promise<void> {
	const name = arguments_.join(" ");
	const specs = commands[name as keyof typeof commands];
	if (specs === undefined) throw new Error(`unknown command '${name}'\n${usage}`);
	for (const spec of specs) {
		const result = await context.exec(spec.file, spec.args, { cwd: context.root });
		context.stdout.write(result.stdout);
		context.stderr.write(result.stderr);
	}
}

/**
 * Parse process arguments and translate execution into a process exit code.
 *
 * @param arguments_ - Complete process argument vector.
 * @param context - Process execution and output dependencies.
 * @returns Zero on success or one when command execution fails.
 */
export async function main(
	arguments_: readonly string[] = process.argv,
	context: CliContext = defaultContext(),
): Promise<number> {
	const command = arguments_.slice(2);
	if (command.length === 0 || command[0] === "--help") {
		context.stdout.write(usage);
		return 0;
	}
	try {
		await runCli(command, context);
		return 0;
	} catch (error) {
		context.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
		return 1;
	}
}

const invokedPath = process.argv[1];
if (invokedPath !== undefined && realpathSync(invokedPath) === fileURLToPath(import.meta.url)) {
	void main().then((exitCode) => {
		process.exitCode = exitCode;
	});
}

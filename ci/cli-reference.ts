//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Checks the committed CLI reference against the built parser.

import {
	action,
	pathInput,
	type ActionInputValues,
	type ScriptExec,
} from "@dedalus-labs/hollywood/action-runtime";

const inputs = {
	bsmr: pathInput({ description: "Built BSMR executable." }),
	expected: pathInput({ description: "Committed CLI reference." }),
} as const;

type Inputs = ActionInputValues<typeof inputs>;
type ReadText = (path: string) => Promise<string>;

/** Reject any difference between generated and committed CLI documentation. */
export function verifyCliReference(expected: string, actual: string): void {
	if (actual !== expected) throw new Error("docs/reference/cli.md is stale; regenerate it from the built BSMR parser");
}

/** Generate and compare the CLI reference through typed process execution. */
export async function checkCliReference(
	exec: ScriptExec,
	readText: ReadText,
	input: Inputs,
): Promise<void> {
	const generated = await exec(input.bsmr, ["docs", "markdown-help-doc", "all"]);
	verifyCliReference(await readText(input.expected), generated.stdout);
}

export const cliReferenceAction = action({
	name: "Check CLI reference",
	description: "Reject CLI documentation that differs from the built parser.",
	localActionPath: "ci/cli-reference",
	inputs,
	outputs: {},
	run: async ({ exec, fs, input }) => {
		await checkCliReference(exec, fs.readText, input);
		return {};
	},
});

//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs the native TypeScript cache and bundled action contract.

import { action, pathInput } from "@dedalus-labs/hollywood/action-runtime";

export const typescriptCache = action({
	name: "Verify TypeScript cache",
	description: "Verify compilation, cache restoration and standalone action execution.",
	localActionPath: "typescript/cache",
	inputs: { binary: pathInput({ description: "BSMR executable path." }) },
	outputs: {},
	run: async ({ exec, input }) => {
		await exec("node", ["test/typescript/cache.ts", input.binary]);
		return {};
	},
});

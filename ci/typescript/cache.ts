//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs native package build and cache contracts.

import { action, pathInput } from "@dedalus-labs/hollywood/action-runtime";

export const typescriptCache = action({
	name: "Verify TypeScript cache",
	description: "Verify package outputs, cache reuse and standalone actions.",
	localActionPath: "typescript/cache",
	inputs: { binary: pathInput({ description: "BSMR executable path." }) },
	outputs: {},
	run: async ({ exec, input }) => {
		await exec("node", ["test/typescript/cache.ts", input.binary]);
		await exec("node", ["test/pnpm/task.ts", input.binary]);
		return {};
	},
});

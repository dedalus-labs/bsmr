//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Embeds checked-in fixture sources into standalone OCI integration runners.

import { defineConfig, type RolldownOptions } from "rolldown";

export default defineConfig([
	["cache", "cache.ts"],
	["tools", "tools.ts"],
	["graph", "graph.ts"],
	["run", "run.ts"],
	["registry", "registry.mjs"],
].map(([name, source]): RolldownOptions => ({
	input: `test/oci/${source}`,
	platform: "node",
	moduleTypes: { ".bzl": "text", ".sh": "text", ".go": "text", ".py": "text", ".mod": "text" },
	transform: { target: "node24" },
	output: { file: `test/oci/dist/${name}.mjs`, format: "esm" },
})));

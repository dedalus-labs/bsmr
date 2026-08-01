//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Bundles Bessemer's local GitHub Actions.

import { defineConfig } from "rolldown";

import { renderPreamble } from "./ci/license-preamble.ts";

const actionPath = ".github/actions/ci/rust-affected/dist/index.js";

export default defineConfig({
	input: "./.github/actions/ci/rust-affected/src/index.ts",
	platform: "node",
	transform: {
		define: { "import.meta.vitest": "undefined" },
		// GitHub Actions executes this bundle with the runtime declared in action.yml.
		target: "node24",
	},
	output: {
		file: actionPath,
		format: "esm",
		minify: true,
		postBanner: renderPreamble(actionPath, "dedalus").trimEnd(),
	},
});

//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies native OCI assembly using the engine built from the selected source.

import { action, choiceInput, pathInput } from "@dedalus-labs/hollywood/action-runtime";
import { releaseVersion } from "../release-version.ts";

export const ociCache = action({
	name: "Verify native OCI cache",
	description: "Compile native Go outputs, verify OCI closure and prove shared cache restoration.",
	localActionPath: "oci/cache",
	inputs: {
		binary: pathInput({ description: "Source-built BSMR executable path." }),
		img: pathInput({ description: "Verified pinned OCI encoder executable path." }),
		platform: choiceInput({ description: "Native Linux target platform.", options: ["linux/amd64", "linux/arm64"] as const }),
	},
	outputs: {},
	run: async ({ exec, input }) => {
		await exec("node", ["test/oci/cache.ts", input.binary, input.img, "prelude", "prelude/oci/operations.mjs",
			"--platform", input.platform, "--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"]);
		return {};
	},
});

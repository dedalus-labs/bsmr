//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies native OCI assembly using the engine built from the selected source.

import { action, choiceInput, pathInput } from "@dedalus-labs/hollywood/action-runtime";
import { releaseVersion } from "../release-version.ts";

export const ociCache = action({
	name: "Verify native OCI cache",
	description: "Verify bundled native OCI rules, cache restoration, and rootful Linux execution.",
	localActionPath: "oci/cache",
	inputs: {
		binary: pathInput({ description: "Source-built BSMR executable path." }),
		img: pathInput({ description: "Verified pinned OCI encoder executable path." }),
		umoci: pathInput({ description: "Verified pinned OCI filesystem tool." }),
		runc: pathInput({ description: "Verified pinned Linux OCI runtime." }),
		registryArchive: pathInput({ description: "Verified Distribution archive for the private TLS fixture." }),
		evidence: pathInput({ description: "Directory for build and native runtime receipts." }),
		platform: choiceInput({ description: "Native Linux target platform.", options: ["linux/amd64", "linux/arm64"] as const }),
	},
	outputs: {},
	run: async ({ exec, input }) => {
		await exec("node", ["test/oci/cache.ts", input.binary, input.img, "prelude", "prelude/oci/operations.mjs",
			"--platform", input.platform, "--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"]);
		await exec("node", ["test/oci/graph.ts", input.binary, input.img, "prelude", "--platform", input.platform,
			"--engine-version", releaseVersion(process.cwd()), "--bundled-prelude", "--artifacts", input.evidence]);
		await exec("node", ["ci/oci/native.ts", input.binary, input.img, input.umoci, input.runc, input.registryArchive, input.platform, input.evidence]);
		return {};
	},
});

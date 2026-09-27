//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies reserved-identity cleanup on a disposable macOS runner.

import { command, expr, job, workflow } from "@dedalus-labs/hollywood";

const paths = ["ci/darwin.ts", "test/sandbox/darwin/**", "test/sandbox/service.py", "tools/native/**", "app/bsmr_sandbox/**", "Cargo.toml"] as const;

export const darwin = workflow({
	name: "macOS isolation",
	on: {
		pull_request: { paths },
		push: { branches: ["main"], paths },
		workflow_dispatch: { inputs: { engine: { type: "boolean", default: false, description: "Build the engine and qualify native action caching." } } },
	},
	permissions: { contents: "read" },
	jobs: {
		lifetime: job({
			name: "Stop detached processes",
			if: expr<boolean>("github.repository == 'dedalus-labs/bsmr' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)"),
			"runs-on": "macos-15",
			"timeout-minutes": 30,
			steps: [
				{ name: "Checkout", uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1", with: { "persist-credentials": false } },
				{ name: "Install Rust", run: command({ file: "rustup", args: ["toolchain", "install", "1.98.0", "--profile", "minimal"] }) },
				{ name: "Restore engine build cache", if: expr<boolean>("github.event_name == 'workflow_dispatch' && inputs.engine"), uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32", env: { CARGO_PROFILE_DEV_DEBUG: "0" }, with: { "prefix-key": "bsmr-native-v1", "shared-key": "engine", "cache-bin": false, "cache-on-failure": true, "cache-workspace-crates": true } },
				{ name: "Build native engine", if: expr<boolean>("github.event_name == 'workflow_dispatch' && inputs.engine"), env: { CARGO_PROFILE_DEV_DEBUG: "0" }, run: command({ file: "cargo", args: ["build", "--locked", "-p", "bsmr", "-j", "2"] }) },
				{ name: "Check native identity ownership", run: command({ file: "cargo", args: ["+1.98.0", "test", "--locked", "--manifest-path", "tools/native/Cargo.toml"] }) },
				{ name: "Check unprivileged refusal", run: command({ file: "cargo", args: ["+1.98.0", "test", "--locked", "--manifest-path", "test/sandbox/darwin/Cargo.toml"] }) },
				{ name: "Build native worker", run: command({ file: "cargo", args: ["+1.98.0", "build", "--release", "--locked", "--manifest-path", "tools/native/Cargo.toml", "--bin", "bsmr-native"] }) },
				{ name: "Build process check", run: command({ file: "cargo", args: ["+1.98.0", "build", "--release", "--locked", "--manifest-path", "test/sandbox/darwin/Cargo.toml"] }) },
				{ name: "Record compiler runtime", run: command({ file: "test/sandbox/darwin/target/release/bsmr-darwin-check", args: ["prepare", "test/sandbox/darwin/target/runtime.txt"] }) },
				{ name: "Check detached process cleanup", run: command({ file: "sudo", args: ["-n", "test/sandbox/darwin/target/release/bsmr-darwin-check", "run", "/private/var/tmp/bsmr-darwin-check", "test/sandbox/darwin/target/runtime.txt"] }) },
				{ name: "Check native service", if: expr<boolean>("github.event_name != 'workflow_dispatch' || !inputs.engine"), run: command({ file: "sudo", args: ["-n", "python3", "test/sandbox/service.py", "tools/native/target/release/bsmr-native", "test/sandbox/darwin/target/release/bsmr-darwin-check", "test/sandbox/darwin/target/runtime.txt"] }) },
				{ name: "Check native build pipeline", if: expr<boolean>("github.event_name == 'workflow_dispatch' && inputs.engine"), run: command({ file: "sudo", args: ["-n", "python3", "test/sandbox/service.py", "tools/native/target/release/bsmr-native", "test/sandbox/darwin/target/release/bsmr-darwin-check", "test/sandbox/darwin/target/runtime.txt", "--engine", "target/debug/bsmr"] }) },
			],
		}),
	},
});

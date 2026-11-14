//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Bounds fixture commands with coreutils while Hollywood owns process execution.

import { nodeExec, type ScriptExec } from "@dedalus-labs/hollywood";

/** Run literal executable arguments with an explicit deadline and forced-stop grace period. */
export function timedExec(seconds: number): ScriptExec {
	return (file, args, options) => nodeExec("timeout", ["--kill-after=5s", `${seconds}s`, file, ...args], options);
}

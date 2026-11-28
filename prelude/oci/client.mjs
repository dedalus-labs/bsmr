//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reaps each owned registry client before callers clean up its files or credentials.

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execute = promisify(execFile);

/** Wait for client closure even when abort rejection arrives before process exit. */
export async function executeClient(binary, args, options) {
	const running = execute(binary, args, options);
	const closed = new Promise((resolve) => running.child.once("close", resolve));
	try {
		return await running;
	} catch (error) {
		// execFile does not forward its timeout killSignal to spawn's abort path.
		if (options.signal?.aborted) running.child.kill("SIGKILL");
		throw error;
	} finally {
		await closed;
	}
}

//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs the checked-in Debian example with its real pinned tool and package acquisition.

import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { machine, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { verifyBundledPrelude } from "./bundled.ts";
import { timedExec } from "./exec.ts";

const { positionals, values } = parseArgs({ allowPositionals: true, options: {
	"engine-version": { type: "string", default: "0.0.9" },
	"bundled-prelude": { type: "boolean", default: false },
} });
const [binary, source, evidencePath] = positionals;
assert.ok(binary && source && evidencePath, "pass engine, repository source, and evidence directory");
assert.equal(positionals.length, 3, "example qualification accepts exactly three paths");
assert.equal(process.platform, "linux");
assert.equal(process.arch, "arm64", "the checked-in tools are pinned for Linux arm64");
assert.equal(machine(), "aarch64", "example qualification cannot use architecture emulation");
assert.equal(process.getuid?.(), 0);
const engine = resolve(binary), repository = resolve(source), evidence = resolve(evidencePath);
mkdirSync(evidence, { recursive: true });
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-debian-example-")));
const workspace = join(root, "workspace");
cpSync(join(repository, "examples/oci/debian"), workspace, { recursive: true });
const execute = timedExec(300);
const options = { cwd: workspace, env: { BSMR_LOCAL_CACHE_DIR: join(root, "cache") } };

try {
	assert.equal((await execute(engine, ["--version"], options)).stdout.trim(), `bsmr ${values["engine-version"]}`);
	if (values["bundled-prelude"]) {
		await verifyBundledPrelude(engine, join(repository, "prelude"), workspace, (file, args) => execute(file, args, options), evidence);
	} else {
		cpSync(join(repository, "prelude"), join(workspace, "prelude"), { recursive: true });
		writeFileSync(join(workspace, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	for (const [phase, target] of [["cold", "//:check"], ["warm", "//:check"], ["export", "//:layout"]]) {
		const report = join(evidence, `${phase}.json`);
		const result = await execute(engine, ["build", target!, "--build-report", report, "--console", "simple", "-v=1,stderr,full_failed_command"], options);
		writeFileSync(join(evidence, `${phase}.log`), result.stdout + result.stderr);
		const build = JSON.parse(readFileSync(report, "utf8"));
		assert.equal(build.success, true);
		const actions = await execute(engine, ["log", "what-ran", "--trace-id", build.trace_id, "--format", "json"], options);
		writeFileSync(join(evidence, `${phase}-actions.jsonl`), actions.stdout);
		if (phase === "warm") assert.equal(actions.stdout.trim(), "");
		if (phase === "export") {
			const layout = resolve(workspace, build.results["root//:layout"].outputs.DEFAULT[0]);
			await execute("tar", ["-cf", join(evidence, "layout.tar"), "-C", layout, "."], options);
		}
		process.stdout.write(`${phase}: ${build.trace_id}\n`);
	}
} catch (error) {
	writeFileSync(join(evidence, "failure.txt"), String(error));
	throw error;
} finally {
	await execute(engine, ["kill"], options);
	rmSync(root, { recursive: true, force: true });
}

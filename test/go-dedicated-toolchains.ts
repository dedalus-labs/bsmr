//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Exercises a root Go test beside a separately declared toolchain cell.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

assert.ok(process.argv[2], "pass the BSMR binary under test");
const binary = resolve(process.argv[2]);
const cwd = mkdtempSync(join(tmpdir(), "bsmr-go-cell-"));
const run = (args: string[]) => execFileSync(binary, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], timeout: 180_000 });
try {
	writeFileSync(join(cwd, "go.mod"), "module example.com/probe\n\ngo 1.26.0\n");
	writeFileSync(join(cwd, "main.go"), "package main\nfunc main() {}\n");
	writeFileSync(join(cwd, "main_test.go"), 'package main\nimport "testing"\nfunc TestRoot(t *testing.T) {}\n');
	run(["init"]);
	const config = join(cwd, ".bsmr");
	writeFileSync(config, readFileSync(config, "utf8").replace("  none = none", "  none = none\n  toolchains = toolchains").replace("  toolchains = root\n", ""));
	mkdirSync(join(cwd, "toolchains"));
	writeFileSync(join(cwd, "toolchains/BUILD.bsmr"), 'load("@prelude//toolchains:native.bzl", "native_tools")\nnative_tools()\n');
	run(["go", "toolchain", "--version", "1.26.1"]);
	run(["go", "sync"]);
	const outputs = JSON.parse(run(["build", "//:bin", "//:test", "-c", "go.link_mode=internal", "--show-full-json-output"]));
	assert.ok(outputs["root//:bin"] && outputs["root//:test"]);
	assert.match(execFileSync(outputs["root//:test"], ["-test.run=^TestRoot$", "-test.v"], { cwd, encoding: "utf8" }), /PASS/);
	console.log("PASS: root Go test with a dedicated toolchain cell");
} finally {
	run(["kill"]);
	rmSync(cwd, { recursive: true });
}

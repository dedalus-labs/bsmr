//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies deterministic helpers used by the Python build-system benchmark.

import assert from "node:assert/strict";
import { test } from "node:test";

import { chmodSync, existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { changedWheelEntries, median, parseOutputputs, performanceGateResults, removeReadOnlyTree, runnerOrder, wheelPayload } from "./helpers.ts";

test("runner order alternates without changing membership", () => {
	assert.deepEqual(runnerOrder(1), ["bsmr", "bazel"]);
	assert.deepEqual(runnerOrder(2), ["bazel", "bsmr"]);
});

test("median selects the middle sorted observation", () => {
	assert.equal(median([9, 1, 5]), 5);
	assert.equal(median([9, 1, 5, 3]), 5);
	assert.throws(() => median([]), /without samples/);
});

test("read-only Bazel tree artifacts can be removed", () => {
	const directory = mkdtempSync(join(tmpdir(), "bsmr-read-only-tree-"));
	const artifact = join(directory, "artifact");
	mkdirSync(artifact);
	writeFileSync(join(artifact, "output.whl"), "wheel");
	chmodSync(artifact, 0o555);
	removeReadOnlyTree(artifact);
	assert.equal(existsSync(artifact), false);
});

test("wheel payload comparison identifies changed and missing paths", () => {
	const original = [{ crc32: 1, name: "django/a.py", size: 1 }, { crc32: 2, name: "django/b.py", size: 2 }];
	const edited = [{ crc32: 3, name: "django/a.py", size: 1 }, { crc32: 4, name: "django/c.py", size: 2 }];
	assert.deepEqual(changedWheelEntries(original, edited), ["django/a.py", "django/b.py", "django/c.py"]);
});

test("performance gates reject regressions and missing paired medians", () => {
	const medians = Object.fromEntries([
		"acquisition-cold",
		"leaf-runtime",
		"leaf-test",
		"leaf-wheel",
		"output-restoration",
		"provisioned-cold",
		"resident-noop",
		"shared-cache-fresh-checkout",
		"test-cached",
		"test-first",
	].flatMap((regime) => [[`${regime}:bsmr`, 1], [`${regime}:bazel`, 10]]));
	const results = performanceGateResults(medians);
	assert.deepEqual(results.map(({ regime }) => regime), [
		"acquisition-cold",
		"leaf-runtime",
		"leaf-test",
		"leaf-wheel",
		"output-restoration",
		"provisioned-cold",
		"resident-noop",
		"shared-cache-fresh-checkout",
		"test-cached",
		"test-first",
	]);
	assert.ok(results.every(({ pass }) => pass));
	medians["resident-noop:bsmr"] = 4;
	assert.equal(performanceGateResults(medians).find(({ regime }) => regime === "resident-noop")?.pass, false);
	assert.throws(() => performanceGateResults({}), /missing positive paired medians/);
});

test("BSMR output parsing requires both semantic artifacts", () => {
	const output = parseOutputputs(JSON.stringify({
		"root//:__bsmr_python_sources": "/tmp/source",
		"root//:django": "/tmp/wheel",
	}));
	assert.deepEqual(output, { source: "/tmp/source", wheel: "/tmp/wheel" });
	assert.throws(() => parseOutputputs("{}"), /omitted/);
});

test("wheel payload reads and filters the ZIP central directory", () => {
	const name = Buffer.from("django/demo.py");
	const central = Buffer.alloc(46 + name.length);
	central.writeUInt32LE(0x02014b50, 0);
	central.writeUInt32LE(0x12345678, 16);
	central.writeUInt32LE(42, 24);
	central.writeUInt16LE(name.length, 28);
	name.copy(central, 46);
	const end = Buffer.alloc(22);
	end.writeUInt32LE(0x06054b50, 0);
	end.writeUInt16LE(1, 8);
	end.writeUInt16LE(1, 10);
	end.writeUInt32LE(central.length, 12);
	end.writeUInt32LE(0, 16);
	const directory = mkdtempSync(join(tmpdir(), "bsmr-wheel-payload-"));
	const wheel = join(directory, "demo.whl");
	writeFileSync(wheel, Buffer.concat([central, end]));
	assert.deepEqual(wheelPayload(wheel, "django/"), [{ crc32: 0x12345678, name: "django/demo.py", size: 42 }]);
	assert.deepEqual(wheelPayload(wheel, "other/"), []);
});

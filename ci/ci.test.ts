import assert from "node:assert/strict";
import test from "node:test";

import { ci } from "./ci.ts";

const jobs = ci.jobs;
const rustLanes = ["rust_audit", "rust_quality", "rust_tests", "rust_self_host"] as const;

test("Rust remains the required aggregate check", () => {
	assert.equal(jobs.rust?.name, "Rust");
	assert.equal(jobs.rust?.["runs-on"], "ubuntu-24.04");
	assert.equal(jobs.rust?.if, "${{ always() }}");
	assert.deepEqual(jobs.rust?.needs, rustLanes);
});

test("Rust compilation uses sized Blacksmith runners", () => {
	assert.equal(jobs.rust_quality?.["runs-on"], "blacksmith-8vcpu-ubuntu-2404");
	assert.equal(jobs.rust_tests?.["runs-on"], "blacksmith-16vcpu-ubuntu-2404");
	assert.equal(jobs.rust_self_host?.["runs-on"], "blacksmith-8vcpu-ubuntu-2404");
	assert.ok(
		jobs.rust_self_host?.steps.some(
			(step) => "run" in step && step.run.includes("--lint-starlark-only"),
		),
	);
});

test("Rust lanes share one trusted cache writer", () => {
	for (const id of ["rust_quality", "rust_tests", "rust_self_host"] as const) {
		const cache = jobs[id].steps.find(
			(step) => "uses" in step && step.uses.startsWith("Swatinem/rust-cache@"),
		);

		assert.ok(cache !== undefined);
		assert.ok("with" in cache);
		assert.ok("shared-key" in cache.with);
		assert.equal(cache.with["shared-key"], "rust");
		assert.equal(
			cache.with["save-if"],
			id === "rust_tests"
				? "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"
				: false,
		);
	}
});

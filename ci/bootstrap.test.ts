//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the shared development bootstrap boundary.

import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
	chmodSync,
	copyFileSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

type BootstrapFixture = Readonly<{
	active: string;
	cache: string;
	cargoLog: string;
	env: NodeJS.ProcessEnv;
	firstRepository: string;
	firstScript: string;
	root: string;
	secondRepository: string;
	secondScript: string;
}>;

const runBootstrap = (script: string, cwd: string, env: NodeJS.ProcessEnv): Promise<string> =>
	new Promise((resolve, reject) => {
		const child = spawn(script, [], { cwd, env });
		let stdout = "";
		let stderr = "";
		child.stdout.on("data", (chunk: Buffer) => (stdout += chunk.toString()));
		child.stderr.on("data", (chunk: Buffer) => (stderr += chunk.toString()));
		child.on("error", reject);
		child.on("close", (code) => {
			if (code === 0) {
				resolve(stdout.trim());
			} else {
				reject(new Error(`bootstrap exited ${code}: ${stderr}`));
			}
		});
	});

const createFixture = (): BootstrapFixture => {
	const fixture = mkdtempSync(join(tmpdir(), "bsmr-bootstrap-"));
	const firstRepository = join(fixture, "repository");
	const secondRepository = join(fixture, "worktree");
	const bin = join(firstRepository, "bin");
	const firstScript = join(firstRepository, "tools", "bootstrap", "bsmr-dev");
	const cargoLog = join(fixture, "cargo.log");
	const cache = join(fixture, "cache");
	const active = join(fixture, "active");
	mkdirSync(dirname(firstScript), { recursive: true });
	mkdirSync(bin);
	mkdirSync(join(firstRepository, ".cargo"));
	copyFileSync(join(root, "tools", "bootstrap", "bsmr-dev"), firstScript);
	chmodSync(firstScript, 0o755);
	writeFileSync(
		join(firstRepository, "rust-toolchain.toml"),
		'[toolchain]\nchannel = "nightly-2026-08-12"\n',
	);
	writeFileSync(join(firstRepository, ".gitignore"), "ignored-input\n");
	writeFileSync(join(firstRepository, ".cargo", "config.toml"), '[build]\nrustflags = ["--cfg", "fixture"]\n');
	writeFileSync(
		join(bin, "rustc"),
		'#!/usr/bin/env bash\necho "${BSMR_TEST_RUSTC_ID:-rustc 1.99.0-nightly}"\n',
	);
	writeFileSync(
		join(bin, "cargo"),
		'#!/usr/bin/env bash\nset -euo pipefail\nif [[ "$*" == *"--version"* ]]; then echo "cargo 1.99.0-nightly"; exit 0; fi\nmkdir "$BSMR_TEST_ACTIVE"\ntrap \'rmdir "$BSMR_TEST_ACTIVE"\' EXIT\nprintf "%s|%s\\n" "$CARGO_TARGET_DIR" "$*" >> "$BSMR_TEST_CARGO_LOG"\nif [[ -n "${BSMR_TEST_MUTATE_FILE:-}" && ! -e "$BSMR_TEST_MUTATE_ONCE" ]]; then printf "changed\\n" > "$BSMR_TEST_MUTATE_FILE"; touch "$BSMR_TEST_MUTATE_ONCE"; fi\nif [[ -n "${BSMR_TEST_MUTATE_TOOL_FILE:-}" && ! -e "$BSMR_TEST_MUTATE_ONCE" ]]; then mkdir -p "$(dirname "$BSMR_TEST_MUTATE_TOOL_FILE")"; printf "[build]\\njobs = 1\\n" > "$BSMR_TEST_MUTATE_TOOL_FILE"; touch "$BSMR_TEST_MUTATE_ONCE"; fi\nif [[ -n "${BSMR_TEST_MUTATE_SELECTED_TOOL:-}" && ! -e "$BSMR_TEST_MUTATE_ONCE" ]]; then printf "#!/usr/bin/env bash\\necho changed\\n" > "$BSMR_TEST_MUTATE_SELECTED_TOOL"; chmod +x "$BSMR_TEST_MUTATE_SELECTED_TOOL"; touch "$BSMR_TEST_MUTATE_ONCE"; fi\nsleep 1\nmkdir -p "$CARGO_TARGET_DIR/debug"\nprintf "#!/usr/bin/env bash\\n" > "$CARGO_TARGET_DIR/debug/bsmr"\nchmod +x "$CARGO_TARGET_DIR/debug/bsmr"\n',
	);
	chmodSync(join(bin, "rustc"), 0o755);
	chmodSync(join(bin, "cargo"), 0o755);
	execFileSync("git", ["init", "-q"], { cwd: firstRepository });
	execFileSync("git", ["add", "."], { cwd: firstRepository });
	execFileSync(
		"git",
		[
			"-c",
			"user.name=BSMR Test",
			"-c",
			"user.email=bsmr-test@example.com",
			"commit",
			"-qm",
			"fixture",
		],
		{ cwd: firstRepository },
	);
	const firstCommit = execFileSync("git", ["rev-parse", "HEAD"], {
		cwd: firstRepository,
		encoding: "utf8",
	}).trim();
	execFileSync(
		"git",
		[
			"-c",
			"user.name=BSMR Test",
			"-c",
			"user.email=bsmr-test@example.com",
			"commit",
			"--allow-empty",
			"-qm",
			"identity-only change",
		],
		{ cwd: firstRepository },
	);
	execFileSync("git", ["worktree", "add", "--detach", "-q", secondRepository, "HEAD"], {
		cwd: firstRepository,
	});
	execFileSync("git", ["switch", "--detach", "-q", firstCommit], { cwd: firstRepository });
	return {
		active,
		cache,
		cargoLog,
		env: {
			...process.env,
			BSMR_BOOTSTRAP_CACHE_DIR: cache,
			BSMR_TEST_ACTIVE: active,
			BSMR_TEST_CARGO_LOG: cargoLog,
			PATH: `${bin}:${process.env["PATH"] ?? ""}`,
		},
		firstRepository,
		firstScript,
		root: fixture,
		secondRepository,
		secondScript: join(secondRepository, "tools", "bootstrap", "bsmr-dev"),
	};
};

test("identical source content compiles one shared development binary", async () => {
	const fixture = createFixture();
	try {
		const [first, second] = await Promise.all([
			runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env),
			runBootstrap(fixture.secondScript, fixture.secondRepository, fixture.env),
		]);

		assert.equal(first, second);
		const [target, args] = readFileSync(fixture.cargoLog, "utf8").trim().split("|");
		assert.equal(target, join(realpathSync(fixture.cache), "targets", first.split("/").at(-3) ?? ""));
		assert.match(
			args ?? "",
			/^\+nightly-2026-08-12 build --locked --manifest-path .*\/source\/Cargo\.toml --bin bsmr$/,
		);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("source mutation during Cargo retries before publication", async () => {
	const fixture = createFixture();
	try {
		const binary = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			BSMR_TEST_MUTATE_FILE: join(fixture.firstRepository, "mutated.rs"),
			BSMR_TEST_MUTATE_ONCE: join(fixture.root, "mutated.once"),
		});
		const builds = readFileSync(fixture.cargoLog, "utf8").split("\n").filter(Boolean);

		assert.equal(builds.length, 2);
		assert.ok(statSync(binary).isFile());
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("source symlinks fail before Cargo execution", async () => {
	const fixture = createFixture();
	try {
		const external = join(fixture.root, "external.rs");
		writeFileSync(external, "external\n");
		symlinkSync(external, join(fixture.firstRepository, "external.rs"));

		await assert.rejects(
			runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env),
			/bootstrap source symbolic links are unsupported/,
		);
		assert.throws(() => statSync(fixture.cargoLog));
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

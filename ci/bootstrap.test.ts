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
	readdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	symlinkSync,
	utimesSync,
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

test("different source content serializes the shared Cargo target", async () => {
	const fixture = createFixture();
	try {
		writeFileSync(join(fixture.secondRepository, "different.rs"), "different\n");
		const [first, second] = await Promise.all([
			runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env),
			runBootstrap(fixture.secondScript, fixture.secondRepository, fixture.env),
		]);

		assert.notEqual(first, second);
		const builds = readFileSync(fixture.cargoLog, "utf8").split("\n").filter(Boolean);
		assert.equal(builds.length, 2);
		assert.equal(builds[0]?.split("|")[0], builds[1]?.split("|")[0]);
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

test("Cargo configuration mutation reroutes publication", async () => {
	const fixture = createFixture();
	try {
		const binary = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			BSMR_TEST_MUTATE_TOOL_FILE: join(fixture.root, ".cargo", "config.toml"),
			BSMR_TEST_MUTATE_ONCE: join(fixture.root, "tool.once"),
		});
		const targets = readFileSync(fixture.cargoLog, "utf8")
			.split("\n")
			.filter(Boolean)
			.map((line) => line.split("|")[0]);

		assert.equal(new Set(targets).size, 2);
		assert.equal(binary.split("/").at(-3), targets[1]?.split("/").at(-1));
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("selected compiler mutation reroutes publication", async () => {
	const fixture = createFixture();
	try {
		const compiler = join(fixture.root, "selected-cc");
		writeFileSync(compiler, "#!/usr/bin/env bash\necho original\n");
		chmodSync(compiler, 0o755);
		const binary = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			CC: compiler,
			BSMR_TEST_MUTATE_SELECTED_TOOL: compiler,
			BSMR_TEST_MUTATE_ONCE: join(fixture.root, "compiler.once"),
		});
		const targets = readFileSync(fixture.cargoLog, "utf8")
			.split("\n")
			.filter(Boolean)
			.map((line) => line.split("|")[0]);

		assert.equal(new Set(targets).size, 2);
		assert.equal(binary.split("/").at(-3), targets[1]?.split("/").at(-1));
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("non-selector compiler controls and empty wrappers are accepted", async () => {
	const fixture = createFixture();
	try {
		const binary = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			CC_ENABLE_DEBUG_OUTPUT: "1",
			CC_SHELL_ESCAPED_FLAGS: "1",
			RUSTC_WRAPPER: "",
		});

		assert.ok(statSync(binary).isFile());
		assert.equal(readFileSync(fixture.cargoLog, "utf8").split("\n").filter(Boolean).length, 1);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("source executable mode selects a distinct binary", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);
		chmodSync(join(fixture.firstRepository, "rust-toolchain.toml"), 0o755);
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);

		assert.notEqual(first, second);
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

test("build environment selects a distinct target and binary", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			RUSTFLAGS: "--cfg first",
		});
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			RUSTFLAGS: "--cfg second",
		});
		const proto = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			BSMR_PROTO_SRCS: "/different/protos",
		});

		assert.notEqual(first, second);
		assert.notEqual(second, proto);
		assert.equal(new Set([first, second, proto].map((path) => path.split("/").at(-3))).size, 3);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("compiler identity selects a distinct target", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			BSMR_TEST_RUSTC_ID: "rustc 2.0.0-nightly",
		});

		assert.notEqual(first, second);
		assert.notEqual(first.split("/").at(-3), second.split("/").at(-3));
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("ignored files are outside the supported source identity", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);
		writeFileSync(join(fixture.firstRepository, "ignored-input"), "not a declared input\n");
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);

		assert.equal(first, second);
		assert.equal(readFileSync(fixture.cargoLog, "utf8").split("\n").filter(Boolean).length, 1);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("interrupted publication temporaries are collected", async () => {
	const fixture = createFixture();
	try {
		const binary = await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env);
		const temporary = join(dirname(binary), "bsmr.tmp.stale");
		const sourceTemporary = join(
			fixture.cache,
			"targets",
			binary.split("/").at(-3) ?? "",
			"source.tmp.stale",
		);
		writeFileSync(temporary, "partial");
		mkdirSync(sourceTemporary);
		writeFileSync(join(sourceTemporary, "partial.rs"), "partial");

		assert.equal(await runBootstrap(fixture.firstScript, fixture.firstRepository, fixture.env), binary);
		assert.ok(statSync(binary).isFile());
		assert.throws(() => statSync(temporary));
		assert.throws(() => statSync(sourceTemporary));
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("target retention preserves fresh leases then removes stale identities", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			RUSTFLAGS: "--cfg first",
			BSMR_BOOTSTRAP_KEEP_TARGETS: "1",
		});
		const secondEnv = {
			...fixture.env,
			RUSTFLAGS: "--cfg second",
			BSMR_BOOTSTRAP_KEEP_TARGETS: "1",
		};
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, secondEnv);
		assert.ok(statSync(first).isFile(), "a freshly returned binary remains leased");

		const firstKey = first.split("/").at(-3) ?? "";
		utimesSync(join(fixture.cache, "targets", firstKey), 1, 1);
		assert.equal(
			await runBootstrap(fixture.firstScript, fixture.firstRepository, {
				...secondEnv,
				BSMR_BOOTSTRAP_MIN_AGE_SECS: "0",
			}),
			second,
		);

		assert.throws(() => statSync(first));
		assert.ok(statSync(second).isFile());
		assert.equal(readdirSync(join(fixture.cache, "targets")).length, 1);
		assert.equal(readdirSync(join(fixture.cache, "tools")).length, 1);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("binary retention bounds source edits within one target", async () => {
	const fixture = createFixture();
	try {
		const first = await runBootstrap(fixture.firstScript, fixture.firstRepository, {
			...fixture.env,
			BSMR_BOOTSTRAP_KEEP_BINARIES: "1",
		});
		writeFileSync(join(fixture.firstRepository, "different.rs"), "different\n");
		const secondEnv = { ...fixture.env, BSMR_BOOTSTRAP_KEEP_BINARIES: "1" };
		const second = await runBootstrap(fixture.firstScript, fixture.firstRepository, secondEnv);
		assert.ok(statSync(first).isFile(), "a freshly returned binary remains leased");

		utimesSync(dirname(first), 1, 1);
		assert.equal(
			await runBootstrap(fixture.firstScript, fixture.firstRepository, {
				...secondEnv,
				BSMR_BOOTSTRAP_MIN_AGE_SECS: "0",
			}),
			second,
		);

		assert.throws(() => statSync(first));
		assert.ok(statSync(second).isFile());
		assert.equal(readdirSync(dirname(dirname(second))).length, 1);
	} finally {
		rmSync(fixture.root, { recursive: true, force: true });
	}
});

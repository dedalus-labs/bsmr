//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs mandatory native OCI qualification against a checked-in platform lock.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { networkInterfaces } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { releaseVersion } from "../release-version.ts";
import { timedExec } from "../../test/oci/exec.ts";

const execute = timedExec(600);
type Command = { file: string; args: string[] };
type NativeOptions = Readonly<{
	binary: string;
	img: string;
	umoci: string;
	runc: string;
	platform: "linux/amd64" | "linux/arm64";
	base: string;
	lock: string;
	evidence: string;
	registryArchive: string;
	bind: string;
}>;

/** Keep rootful qualification mandatory and execute the source-built engine's bundled rules. */
export function nativeCommands({ binary, img, umoci, runc, platform, base, lock, evidence, registryArchive, bind }: NativeOptions): readonly Command[] {
	return [
		{ file: "sudo", args: ["-n", "env", "BSMR_OCI_REQUIRE_NATIVE=1", `BSMR_OCI_PLATFORM=${platform}`,
			`BSMR_OCI_RUN_BASE=${base}`, `BSMR_OCI_UMOCI=${umoci}`, `BSMR_OCI_RUNC=${runc}`,
			process.execPath, "--test", "test/oci/run.test.mjs"] },
		{ file: "sudo", args: ["-n", "env", `BSMR_OCI_TEST_EVIDENCE=${evidence}`, process.execPath,
			"test/oci/run.ts", binary, img, umoci, runc, base, lock, "prelude",
			"--platform", platform, "--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"] },
		{ file: "sudo", args: ["-n", "env", process.execPath, "test/oci/registry.mjs", "--bsmr", binary,
			"--img", img, "--registry-archive", registryArchive, "--layout", base, "--prelude", resolve("prelude"),
			"--bind", bind, "--evidence", evidence, "--platform", platform,
			"--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"] },
	];
}

/** Save each command's bounded receipt while propagating acquisition or runtime failures. */
async function record(root: string, name: string, command: Command) {
	try {
		const result = await execute(command.file, command.args);
		writeFileSync(join(root, `${name}.log`), result.stdout + result.stderr);
		process.stdout.write(result.stdout);
		process.stderr.write(result.stderr);
	} catch (error) {
		writeFileSync(join(root, `${name}-failure.log`), String(error));
		throw error;
	}
}

/** Acquire the exact base once, then run strict native tests and the bundled package graph. */
async function main() {
	const [binary, img, umoci, runc, registryArchive, platform, evidence] = process.argv.slice(2);
	assert.ok(binary && img && umoci && runc && registryArchive && platform && evidence && process.argv.length === 9);
	assert.equal(process.platform, "linux", "native OCI CI requires Linux");
	assert.ok(process.getuid && process.getgid);
	assert.equal(platform, `linux/${process.arch === "x64" ? "amd64" : process.arch}`, "native OCI CI cannot emulate its target architecture");
	assert.ok(platform === "linux/amd64" || platform === "linux/arm64");
	const bind = Object.values(networkInterfaces()).flatMap((entries) => entries ?? [])
		.filter((entry) => entry.family === "IPv4" && !entry.internal).map((entry) => entry.address).sort()[0];
	assert.ok(bind, "registry qualification requires an assigned non-loopback IPv4 address");
	mkdirSync(resolve(evidence), { recursive: true });
	const root = mkdtempSync(join(resolve(evidence), "native-"));
	const imageLock = resolve(platform === "linux/amd64" ? "test/oci/fixtures/linux-amd64.image.lock.json" : "examples/oci/debian/image.lock.json");
	const packageLock = resolve(platform === "linux/amd64" ? "test/oci/fixtures/linux-amd64.debian.lock.json" : "examples/oci/debian/debian.lock.json");
	const locked = JSON.parse(readFileSync(imageLock, "utf8"));
	assert.equal(locked.platform, platform);
	const spec = join(root, "pull.json"), base = join(root, "base");
	writeFileSync(spec, JSON.stringify({ image: locked.image, platform, lock: imageLock }));
	await record(root, "acquire-base", { file: process.execPath, args: ["prelude/oci/sources.mjs", "--img", resolve(img),
		"--spec", spec, "--output", base, "--manifest", join(root, "manifest.json"),
		"--config", join(root, "config.json"), "--descriptor", join(root, "descriptor.json")] });
	try {
		const phases = ["runtime-contracts", "bundled-package-graph", "private-registry"];
		const commands = nativeCommands({ binary: resolve(binary), img: resolve(img), umoci: resolve(umoci), runc: resolve(runc),
			platform, base, lock: packageLock, evidence: root, registryArchive: resolve(registryArchive), bind });
		for (const [index, command] of commands.entries()) {
			const phase = phases[index];
			assert.ok(phase, "each native qualification command requires a receipt name");
			await record(root, phase, command);
		}
	} finally {
		// Root-created mkdtemp directories otherwise hide receipts from artifact upload.
		await execute("sudo", ["-n", "chown", "-R", "--no-dereference", `${process.getuid()}:${process.getgid()}`, root]);
	}
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();

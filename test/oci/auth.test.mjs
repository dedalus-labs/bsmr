//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves registry credentials cannot select another host or ambient identity.

import assert from "node:assert/strict";
import test from "node:test";
import { registryEnvironment } from "../../prelude/oci/auth.mjs";

const registry = "registry.example.com:5443";
const credentials = { IMG_REGISTRY_AUTH_HOST: registry, IMG_REGISTRY_AUTH_USERNAME: "fixture", IMG_REGISTRY_AUTH_PASSWORD: "fixture-password" };

test("invariant anonymous registry operations cannot discover ambient credentials", () => {
	const env = registryEnvironment(registry, "/private/auth");
	assert.equal(env.DOCKER_CONFIG, "/private/auth");
	assert.equal(env.PATH, "/nonexistent");
	assert.equal(env.GOOGLE_APPLICATION_CREDENTIALS, "/dev/null");
	assert.equal(env.AWS_EC2_METADATA_DISABLED, "true");
	assert.equal(env.AWS_SHARED_CREDENTIALS_FILE, "/dev/null");
	assert.equal(env.AWS_CONFIG_FILE, "/dev/null");
	assert.equal(env.AWS_ECR_DISABLE_CACHE, "true");
	assert.equal(env.IMG_INSECURE, "0");
	assert.equal(Object.keys(env).some((name) => name.startsWith("IMG_REGISTRY_AUTH_")), false);
});

test("invariant explicit credentials carry only one host-scoped authentication mode", () => {
	for (const auth of [credentials, { IMG_REGISTRY_AUTH_HOST: registry, IMG_REGISTRY_AUTH_BEARER_TOKEN: "fixture-token" }]) {
		const env = registryEnvironment(registry, "/private/auth", { ...auth,
			AWS_SECRET_ACCESS_KEY: "must-not-escape", IMG_CREDENTIAL_HELPER: "/untrusted/helper",
			IMG_DOCKER_CONFIG_INLINE: "must-not-escape", IMG_INSECURE: "1", IMG_AUTH_DEBUG: "1", SSL_CERT_FILE: "/private/registry-ca.pem" });
		assert.deepEqual(Object.fromEntries(Object.entries(env).filter(([name]) => name.startsWith("IMG_REGISTRY_AUTH_"))), auth);
		assert.equal(env.SSL_CERT_FILE, "/private/registry-ca.pem");
		assert.equal(env.IMG_INSECURE, "0");
		assert.equal(JSON.stringify(env).includes("must-not-escape"), false);
		assert.equal(env.IMG_CREDENTIAL_HELPER, undefined);
		assert.equal(env.IMG_AUTH_DEBUG, undefined);
	}
});

test("invariant missing mixed mismatched or malformed credentials fail without their values", () => {
	for (const patch of [{ IMG_REGISTRY_AUTH_HOST: "another.example.com" }, { IMG_REGISTRY_AUTH_PASSWORD: "" },
		{ IMG_REGISTRY_AUTH_USERNAME: undefined }, { IMG_REGISTRY_AUTH_BEARER_TOKEN: "fixture-token" },
		{ IMG_REGISTRY_AUTH_PASSWORD: "fixture-secret\n" }, { IMG_REGISTRY_AUTH_EXTRA: "fixture-secret" },
		{ SSL_CERT_FILE: "relative.pem" }]) {
		assert.throws(() => registryEnvironment(registry, "/private/auth", { ...credentials, ...patch }),
			(error) => error.code === "OCI_REGISTRY_AUTH" && !error.message.includes("fixture-secret"));
	}
	assert.throws(() => registryEnvironment(registry, "/private/auth", {}), { code: "OCI_REGISTRY_AUTH" });
});

test("invariant registry hosts cannot contain URLs credentials or path components", () => {
	for (const host of ["https://registry.example.com", "user:password@registry.example.com", "registry.example.com/path", "registry.example.com?query", "registry.example.com#fragment", "REGISTRY.EXAMPLE.COM"]) {
		assert.throws(() => registryEnvironment(host, "/private/auth"), { code: "OCI_REGISTRY_AUTH" });
	}
});

test("invariant registry names cannot activate the client's plaintext transport", () => {
	for (const host of ["localhost", "localhost:5000", "registry.localhost:5000", "127.0.0.1:5000",
		"10.0.0.1:5000", "172.16.0.1:5000", "192.168.1.1:5000", "[::1]:5000", "203.0.113.1:5000"]) {
		assert.throws(() => registryEnvironment(host, "/private/auth"), { code: "OCI_REGISTRY_AUTH" });
	}
});

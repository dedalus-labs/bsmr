//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Restricts registry clients to anonymous access or one explicit runtime credential.

import { isAbsolute } from "node:path";
import { isIP } from "node:net";

const authFields = ["IMG_REGISTRY_AUTH_HOST", "IMG_REGISTRY_AUTH_USERNAME", "IMG_REGISTRY_AUTH_PASSWORD", "IMG_REGISTRY_AUTH_BEARER_TOKEN"];

/** Name authentication errors without including credential values. */
function fail(message) {
	throw Object.assign(new Error(message), { code: "OCI_REGISTRY_AUTH" });
}

/** Keep host files, cloud metadata, helpers, and transport overrides outside registry operations. */
export function registryEnvironment(registry, scratch, credentials = null) {
	let url;
	try { url = new URL(`https://${registry}/`); } catch { fail("registry must be a host with an optional port"); }
	if (url.host !== registry || url.pathname !== "/" || url.username || url.password || url.search || url.hash) {
		fail("registry must be a canonical host with an optional port");
	}
	// The pinned client's local-address policy can retry HTTPS failures over HTTP.
	// Require DNS names so credentials never enter that downgrade path.
	if (isIP(url.hostname) || url.hostname.startsWith("[") || url.hostname === "localhost" || url.hostname.endsWith(".localhost")) {
		fail("registry must use a DNS hostname that requires HTTPS, not an IP literal or localhost");
	}
	const env = {
		LANG: "C", TZ: "UTC", PATH: "/nonexistent", DOCKER_CONFIG: scratch, IMG_INSECURE: "0",
		// img's cloud keychains must not discover files or machine credentials after anonymous lookup.
		GOOGLE_APPLICATION_CREDENTIALS: "/dev/null", AWS_EC2_METADATA_DISABLED: "true",
		AWS_SHARED_CREDENTIALS_FILE: "/dev/null", AWS_CONFIG_FILE: "/dev/null", AWS_ECR_DISABLE_CACHE: "true",
	};
	if (credentials === null) return env;
	if (Object.keys(credentials).some((name) => name.startsWith("IMG_REGISTRY_AUTH_") && !authFields.includes(name))) {
		fail("unknown IMG_REGISTRY_AUTH_ field");
	}
	if (credentials.IMG_REGISTRY_AUTH_HOST !== registry) fail("IMG_REGISTRY_AUTH_HOST must exactly match the requested registry");
	const user = credentials.IMG_REGISTRY_AUTH_USERNAME;
	const password = credentials.IMG_REGISTRY_AUTH_PASSWORD;
	const bearer = credentials.IMG_REGISTRY_AUTH_BEARER_TOKEN;
	if (bearer ? user || password : !user || !password) fail("set either a bearer token or both username and password");
	for (const name of authFields) {
		const value = credentials[name];
		if (value === undefined || value === "") continue;
		if (typeof value !== "string" || /[\0\r\n]/u.test(value)) fail("credential fields must be single-line strings");
		env[name] = value;
	}
	// Explicit runtime acquisition/publication may use a private registry's CA, never insecure TLS.
	for (const name of ["SSL_CERT_FILE", "SSL_CERT_DIR"]) {
		const value = credentials[name];
		if (value === undefined) continue;
		if (typeof value !== "string" || !isAbsolute(value) || /[\0\r\n]/u.test(value)) fail("TLS trust paths must be absolute");
		env[name] = value;
	}
	return env;
}

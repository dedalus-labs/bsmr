<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the repository security policy. -->

# Security

Report vulnerabilities through
[GitHub private vulnerability reporting](https://github.com/dedalus-labs/bsmr/security/advisories/new).
Do not open a public issue.

Only the latest commit on `main` receives security fixes.

## CI trust boundary

Pull requests never receive write permissions or repository secrets. The
`CLA` and `Vouch` jobs check the author against `VOUCHED.td` from the trusted
base commit. Full CI runs only for the canonical repository and same-repository
pull request branches; fork pull requests cannot consume build runners.

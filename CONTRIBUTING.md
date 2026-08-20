<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Contributing to Bessemer

Thanks for helping improve Bessemer.

## Getting vouched

Bessemer accepts external contributions from vouched contributors. Being
listed in `VOUCHED.td` records that a maintainer verified your GitHub account
and your acceptance of the [CLA](CLA.md).

1. Open a "Vouch request" issue.
2. Confirm that you have read and accept `CLA.md`.
3. Link public work or ask an existing vouched contributor to sponsor you.
4. Wait for a maintainer to add your GitHub handle to `VOUCHED.td`.

Do not add yourself to `VOUCHED.td`. That file is maintainer-owned trust state.
Fork pull requests run only the contributor checks. A maintainer moves vouched
work to a repository branch before running the full CI suite.

## Pull requests

Keep each pull request focused on one behavior. Explain why the change is
needed and include the commands that prove it works.

Before opening a pull request:

```sh
cargo fmt --all -- --check
cargo build --locked --bin bsmr
python3 test.py --ci --git --bsmr=target/debug/bsmr
pnpm install --frozen-lockfile --ignore-scripts
pnpm run ci check
```

Update tests when behavior changes. Update documentation when a public
interface changes.

Repository policy is managed with Terraform from Dedalus's protected
repository-controls stack. Do not change GitHub settings manually.

Report vulnerabilities through the private process in
[`SECURITY.md`](SECURITY.md), never through a public issue.

## Releases

Release Please owns release pull-request titles, bodies, changelogs, and
versions. Do not rename or manually edit a release pull request. Release
Please parses the merged title when creating the GitHub release, so a manual
rename can produce a tag that disagrees with the reviewed version files.

To correct an open release candidate:

1. Do not merge or replace the existing release pull request.
2. If only its title or body was edited, run the `Release Please` workflow on
   `main` and let the bot restore the pull request.
3. If the proposed version is wrong, add the intended version as `release-as`
   under `packages["."]` in `release-please-config.json`, merge that fix, and
   run `Release Please` on `main`.
4. Wait for the existing release pull request to update. The release sync
   removes the one-shot `release-as` setting and synchronizes `VERSION`,
   `.release-please-manifest.json`, `app/bsmr/Cargo.toml`, `Cargo.lock`, and
   `dist-workspace.toml`.
5. Verify those files, the changelog, the pull-request title, and all required
   checks before merging.

If the incorrect release pull request already merged, inspect the tag and
GitHub Release before changing anything. Never delete or reuse a published
version. Correct it with a new, unused version through the same process.

## License

Unless explicitly stated otherwise, contributions submitted to Bessemer are
licensed under the [Apache License 2.0](LICENSE). Preserve every
copyright, license, and attribution notice in inherited files.

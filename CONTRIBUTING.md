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

## License

Contributions are licensed under both the [MIT license](LICENSE-MIT) and the
[Apache License 2.0](LICENSE-APACHE).

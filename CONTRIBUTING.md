# Contributing to Bessemer

Thanks for helping improve Bessemer.

## Pull requests

Keep each pull request focused on one behavior. Explain why the change is
needed and include the commands that prove it works.

Before opening a pull request:

```sh
cargo fmt --all -- --check
cargo build --locked --bin bsmr
python3 test.py --ci --git --bsmr=target/debug/bsmr
pnpm install --frozen-lockfile --ignore-scripts
pnpm run ci:check
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

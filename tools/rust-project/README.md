# rust-project

`rust-project` reads dependency information from Buck and generates a
[rust-project.json](https://rust-analyzer.github.io/manual.html#non-cargo-based-projects)
file for `rust-analyzer`.

# Usage

Run the tool from the repository root with one or more Buck targets:

```bash
./tools/bin/rust-project develop //app/bsmr:bsmr
```

The command writes `rust-project.json` to the current directory, where
`rust-analyzer` can discover it.

To emit logs, set the environment variable `RUST_LOG` to a value. Supported
syntax is described
[here](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html).

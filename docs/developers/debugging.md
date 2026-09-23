<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

This page contains generic debugging advice for developers of Bessemer; this advice is descriptive
(based on what people usually do today) not prescriptive (you're welcome to come up with your own
ideas).

## Normal logic bugs

We usually debug normal logic bugs by looking at the code, writing finer grained tests, or standard
`println!` debugging.

Use of a traditional debugger is not common, but it probably works using standard tools in OSS<FbInternalOnly>,
internally see [debuggers_internally](./debuggers_internally.fb.md) if you want to try</FbInternalOnly>.

Bessemer has many commands to retrieve information about the build, particularly `bsmr log`, `bsmr
audit` and `bsmr debug`, which can be helpful.

## Running builds

Run the source build through Cargo from the repository root:

```sh
cargo run --locked --bin bsmr -- --isolation-dir=dev --help
```

Replace `--help` with the command you want to debug. Keep `--isolation-dir=dev`
to give this build its own daemon and output directory. This separates it from
your installed Bessemer daemon and changes cache keys that contain output paths.

To run the binary in another checkout, use `cargo build --locked --bin bsmr`,
then invoke the absolute path to `target/debug/bsmr` from that checkout.
On Windows, the binary is `target/debug/bsmr.exe`.

## Logging

bsmr emits most of its logs in a structured form that is best interacted with via `bsmr logs`
commands.

We additionally have some tracing logging, though it's sparse and not in very widespread use. Use
the `BSMR_LOG` environment variable to enable trace logging. Requires daemon restart:

```bash
bsmr kill
BSMR_LOG=module_name=trace bsmr <command>
# Example
BSMR_LOG=starlark=trace bsmr uquery cell//path/to:target
BSMR_LOG=bsmr_execute_impl::materializers=trace bsmr build cell//path/to:target
```

Use `cargo run --locked --bin bsmr -- --isolation-dir=dev <command>` to run local
changes. Restart only that daemon with the same prefix followed by `kill`.

See
[tracing-subscriber docs](https://docs.rs/tracing-subscriber/0.2.17/tracing_subscriber/filter/struct.EnvFilter.html)
for filter syntax.

<FbInternalOnly>

### Investigating configuration transitions

If you're trying to work out where a transition happens within a dependency
chain, you may find the following script useful:

```sh
scripts/torozco/parse_deps
```

## Making a change to Bessemer Tpx

Bessemer invokes Tpx when running tests. If you're changing Tpx, you can build your
own Tpx and then have Bessemer use it, as follows:

```bash
# Build Tpx
bsmr build @upstream//mode/opt root//bsmr_tpx_cli:bsmr_tpx_cli --out /tmp/tpx

# Use Tpx
bsmr test -c test.v2_test_executor=/tmp/tpx
```

To get access to Tpx's stderr and stdout if you are print-debugging, you need to also get Bessemer to have the right log level for it:

```sh
BSMR_LOG=bsmr_test=debug bsmr test
```

Remember that you need a daemon restart to change `BSMR_LOG`.

Refer to the [tpx wiki](https://www.internalfb.com/wiki/TAE/tpx/Hacking_on_Tpx/) for more details.

</FbInternalOnly>

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

`./bsmr.py <command>` builds bsmr from source and runs `<command>` using that bsmr. The command
is run in a different isolation dir, to prevent the command from stepping on your existing buck
daemon. This means large builds will get no cache hits and be very slow.

Alternatively, `bsmr build @upstream//mode/opt root//:bsmr --out /tmp/bsmr` to build bsmr
on its own. Then, use `/tmp/bsmr` to run builds in a *different* checkout of fbsource from the one
you're editing code in.

## Logging

bsmr emits most of its logs in a structured form that is best interacted with via `bsmr logs`
commands.

We additionally have some tracing logging, though it's sparse and not in very widespread use. Use
the `BUCK_LOG` environment variable to enable trace logging. Requires daemon restart:

```bash
bsmr kill
BUCK_LOG=module_name=trace bsmr <command>
# Example
BUCK_LOG=starlark=trace bsmr uquery cell//path/to:target
BUCK_LOG=bsmr_execute_impl::materializers=trace bsmr build cell//path/to:target
```

Or use `./bsmr.py` instead of `bsmr` to run local changes

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

Alternatively, you can build buck and tpx in one go with `fbcode/bsmr/bsmr.py` and use it like buck:

```sh
fbcode/bsmr/bsmr.py test ...
```

To get access to Tpx's stderr and stdout if you are print-debugging, you need to also get Bessemer to have the right log level for it:

```sh
BUCK_LOG=bsmr_test=debug bsmr test
```

Remember that you need a daemon restart to change `BUCK_LOG`.

Refer to the [tpx wiki](https://www.internalfb.com/wiki/TAE/tpx/Hacking_on_Tpx/) for more details.

</FbInternalOnly>

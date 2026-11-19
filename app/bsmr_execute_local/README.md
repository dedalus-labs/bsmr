<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines local process stream ownership. -->

# Local command execution

`spawn_command_and_stream_events` starts one command and returns its output and
terminal status through one event stream. The caller keeps polling that stream
until completion. Dropping an unfinished stream does not establish cleanup.

```text
Command + CommandIo -> process -> stdout/stderr -> terminal status
                       |                              |
                  cancellation                  StatusDecoder
```

`CommandIo` owns the selected stdin descriptor and optional output redirects.
Its default uses an empty input and captures both output streams. A caller can
instead supply a request file or socket. Process creation consumes that input.
Existing command callers select the default explicitly.

`StatusDecoder` observes normal completion or cancellation before the terminal
event. A scoped executor can use that boundary to finish required cleanup.
Process-group termination alone does not stop descendants that create another
session. Native sandbox backends must enforce their stronger lifetime contract.

Run the real process and stream tests with:

```sh
cargo test --locked -p bsmr_execute_local --lib
```

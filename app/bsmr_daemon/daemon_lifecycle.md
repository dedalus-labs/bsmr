<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# bsmrd

Bsmr runs a persistent daemon process (bsmrd) to reuse work between commands.
Most work is done by the daemon process. When executing a bsmr command, the
process running the command is a client to the bsmrd server. The bsmrd server
exposes a simple grpc service that the client uses to implement the various bsmr
commands.

There's a small set of commands/arguments that don't require the daemon
(`bsmr help`, cli arg parse failures, `bsmr version`, ...), but most commands
will require it.

For almost all commands, bsmr requires that the client and server are the same
version of bsmr and may restart bsmrd to ensure that's the case.

# daemon process flow

The daemon process is started with the (hidden) `bsmr daemon` command.

The daemon process has a simple startup. It will first daemonize itself and
write its pid to a locked file "bsmrd.pid" in the "daemon directory" (a
directory in `$HOME/.bsmr` specific to that repository+output directory). The
file is locked exclusively by the daemon process until it exits. This means that
only a single daemon is allowed at a time. It redirects its stdout and stderr to
files in the daemon directory.

The daemon then starts up the grpc DaemonApi server. Once that is running, it
will write the port it is running on (along with some other information) to the
"bsmrd.info" file in the daemon dir. Once that is done, the server is ready to
be used.

There are 3 ways that the bsmrd process will shutdown:

1. The grpc api includes a `kill()` call that will shutdown bsmrd.
2. bsmrd will periodically (every 100s or so) check the "bsmrd.pid" and
   "bsmrd.info" files to ensure that they still match that bsmrd process.
3. If bsmrd hits a rust `panic()` the bsmrd process will exit

# client connection and bsmrd startup

When the client is processing a command that requires communicating with the
bsmrd server it will follow this approach:

1. read the "bsmrd.info" file to get the port the grpc api is being served on
2. connect to the api on that port
3. send a `status()` request to get the version

If there is an error during 1-3, or if there is a version mismatch the client
needs to (re)start the bsmr daemon. Otherwise, the client can continue as it now
has made a connection with a correctly versioned bsmrd.

When the client is killing or starting the bsmrd process, it will grab an
exclusive lock on the "lifecycle.lock" file in the daemon directory to ensure
that multiple clients aren't racing with each other.

To start/restart the bsmrd process, the client does:

1. lock the "lifecycle.lock" file
2. send a kill command to the existing bsmrd
3. ensure the bsmrd process has exited (based on pid)
4. run a `bsmr daemon` command to start bsmrd
5. wait for the daemon to start up and the grpc server to be ready
6. release the "lifecycle.lock" file

After that, it will repeat the connection steps (including verifying the version
after connecting).

# bsmr kill and other daemon restarts

If there are other invocations currently using the bsmr daemon when it is killed
or restarted by a client, those invocations will fail due to the early
disconnection.

Generally, we support concurrent bsmr invocations using the same bsmr version,
but if there are concurrent invocations with different versions, they may
unexpectedly fail or otherwise work incorrectly. This is sufficient for the
normal bsmr workflow where the bsmrversion is checked into the repo, in that
case, it's not expected that bsmr commands will work across a rebase or other
operation that changes the bsmrversion.

# correctness

We have a couple of guarantees here.

1. Only a single bsmrd is running at a time
2. Only a single client is killing/starting a bsmrd at a time
3. A client only uses a bsmrd connection after making sure it has a compatible
   version

The main way that we could run into issues would be if there are multiple
clients that are racing and they want different versions of bsmr. In that case,
one might cause the other two fail to connect to a bsmrd with the correct
version or one of the client's connections may be prematurely disconnected. A
client **will not** use a server with a mismatched version. While this is a
failure, no expected workflow would hit this case, all concurrent commands
should be using the same bsmr version.

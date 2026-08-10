---
id: what_is_bsmr
title: What is Bessemer?
---

Bessemer (`bsmr`) is a build system for repositories that contain many
languages and many dependent targets.

It provides:

- an incremental dependency graph;
- local and remote action execution;
- query commands for inspecting configured and unconfigured targets;
- Starlark rules that keep language support outside the core binary; and
- a shared prelude for common language toolchains.

Bessemer is derived from
[Buck2](https://github.com/facebook/buck2). Except where an inherited notice
states otherwise, Bessemer is licensed under Apache-2.0. Buck2-derived and
third-party files retain their original copyright, license, and attribution
notices.

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
[Buck2](https://github.com/facebook/buck2). The project retains upstream
copyright notices and licenses while developing under its own name and public
interfaces.

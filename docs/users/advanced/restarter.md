---
id: restarter
title: Restarter
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


The Restarter can automatically restart Bessemer when Bessemer detects that it hit a
condition that may be recovered by restarting the Bessemer daemon.

This is particularly useful with
[Deferred Materialization](deferred_materialization.md), which may require a
daemon restart if your daemon holds references to artifacts that have expired in
your Remote Execution backend.

## Enabling the Restarter

To enable, add this to your Bsmrconfig:

```ini
[bsmr]
restarter = true
```

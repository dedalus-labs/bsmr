---
id: in_memory_cache
title: In Memory Cache
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


Bessemer maintains an in-memory cache of actions it executed. This allows actions
to skip re-running even when they are (transitively) affected by file changes.

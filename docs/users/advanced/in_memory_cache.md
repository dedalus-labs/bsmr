---
id: in_memory_cache
title: In Memory Cache
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


Bessemer maintains an in-memory cache of actions it executed. This allows actions
to skip re-running even when they are (transitively) affected by file changes.

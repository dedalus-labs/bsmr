---
id: third_party_packages
title: Third-Party Packages
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Third-Party Packages

Bessemer treats third-party packages as ordinary graph nodes and action inputs.
The native frontend currently requires a checked-in vendor tree so frozen graph
discovery and builds cannot read a mutable module cache or contact a proxy.

Use the ordinary Go workflow at the explicit dependency-update boundary:

```shell
go get example.com/module@v1.2.3
go mod tidy
go mod vendor
bsmr go toolchain
bsmr go sync
```

Commit `go.mod`, `go.sum`, `vendor/modules.txt`, the selected vendor sources,
`.bsmr-go-manifests`, and generated Bessemer manifests. CI should run:

```shell
bsmr go toolchain --check
bsmr go sync --check
```

During synchronization Bessemer selects `-mod=vendor` when
`vendor/modules.txt` exists. It otherwise uses read-only module mode, disables
the proxy and checksum database, and rejects any package returned outside the
repository root. A missing vendored package is therefore a precise error, not a
network request or an implicit fallback.

Verified module acquisition directly into the CAS is a later RFC 0003
milestone. Until that lands, vendoring is the only supported third-party source
boundary for native synchronization.

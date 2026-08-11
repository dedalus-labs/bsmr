<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

## Remote execution integration with EngFlow

This project provides a small example of what a project that utilizes
[EngFlow](https://www.engflow.com/)'s RE offering might look like.

In this document, we will go over the key configs used in this setup.

### Relevant configs in .bsmrconfig

First, the EngFlow endpoint and certificate should be configured as the
following:

```ini
[bsmr_re_client]
engine_address       = $ENGFLOW_ENDPOINT
action_cache_address = $ENGFLOW_ENDPOINT
cas_address          = $ENGFLOW_ENDPOINT
tls_client_cert      = $ENGFLOW_CERTIFICATE
```

Additionally, set the `digest_algorithm` config to `SHA256`.

```ini
[bsmr]
digest_algorithms = SHA256
```

### Relevant configs in `ExecutionPlatformInfo`

EngFlow takes in a Docker image as its execution platform. The execution
platform used in this project `root//platforms:platforms` uses the
`container-image` key to set this up.

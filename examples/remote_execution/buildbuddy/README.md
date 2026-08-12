<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

## Remote execution integration with BuildBuddy

This project provides a small example of what a project that utilizies
[BuildBuddy](https://www.buildbuddy.io/)'s RE might look like.

In this document, we will go over the key configs used in this setup.

### Relevant configs in .bsmr

First, the BuildBuddy endpoint and api key should be configured as the
following:

```ini
[bsmr_re_client]
engine_address       = $BUILDBUDDY_ENDPOINT
action_cache_address = $BUILDBUDDY_ENDPOINT
cas_address          = $BUILDBUDDY_ENDPOINT
http_headers         = x-buildbuddy-api-key:$BUILDBUDDY_API_KEY
```

### Relevant configs in `ExecutionPlatformInfo`

BuildBuddy takes in a Docker image and OSFamily in its execution platform's
execution properties(`exec_properties`) to select an executor. The execution
platform used in this project `root//platforms:platforms` uses the
`container-image` key to set this up.

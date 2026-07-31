# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//:native.bzl", _native = "native")
load(
    "@prelude//utils:bsmrconfig.bzl",
    _read_config = "read_config_with_logging",
    _read_root_config = "read_root_config_with_logging",
    log_bsmrconfigs = "LOG_BSMRCONFIGS",
)

__overridden_builtins__ = (
    {
        "read_config": _read_config,
        "read_root_config": _read_root_config,
    }
    if log_bsmrconfigs
    else {}
)

load_symbols(__overridden_builtins__)

# Public symbols in this file become globals everywhere except `bzl` files in prelude.
# Additionally, members of `native` struct also become globals in `BUCK` files.
native = _native

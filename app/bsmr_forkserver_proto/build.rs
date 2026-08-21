//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::env;
use std::io;

fn main() -> io::Result<()> {
    let proto_files = &["forkserver.proto"];

    let bsmr_proto_srcs = env::var("BSMR_PROTO_SRCS");
    let includes = if let Ok(path) = &bsmr_proto_srcs {
        vec![path.as_str()]
    } else {
        vec![".", "../bsmr_data", "../bsmr_host_sharing_proto"]
    };

    let builder = bsmr_protoc_dev::configure();
    unsafe { builder.setup_protoc() }
        .type_attribute(
            "bsmr.forkserver.RequestEvent.data",
            "#[derive(::derive_more::From, ::gazebo::variants::VariantName, ::gazebo::variants::UnpackVariants)]",
        )
        .type_attribute(
            "bsmr.forkserver.EnvDirective.data",
            "#[derive(::derive_more::From, ::gazebo::variants::VariantName, ::gazebo::variants::UnpackVariants)]",
        )
        .type_attribute(
            "bsmr.forkserver.RequestEvent.data",
            "#[allow(clippy::large_enum_variant)]",
        )
        .extern_path(".bsmr.data", "::bsmr_data")
        .compile(proto_files, &includes)
}

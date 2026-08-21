//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Generates Rust bindings for Bessemer's stable build-event protocol.

use std::env;
use std::io;

/// Generates the public build-event Rust bindings.
fn main() -> io::Result<()> {
    let proto_files = &["build_event.proto"];
    let proto_srcs = env::var("BSMR_PROTO_SRCS");
    let includes = proto_srcs.as_deref().map_or(vec!["."], |path| vec![path]);

    let builder = bsmr_protoc_dev::configure();
    unsafe { builder.setup_protoc() }
        .type_attribute(".", "#[derive(::serde::Serialize, ::serde::Deserialize)]")
        .type_attribute(
            "bsmr.build.v1.BuildEvent.payload",
            "#[serde(rename_all = \"snake_case\")]",
        )
        .field_attribute(
            "bsmr.build.v1.TestAttemptCompleted.outcome",
            "#[serde(with = \"crate::serde_test_outcome\")]",
        )
        .field_attribute(
            "bsmr.build.v1.TestAttemptCompleted.execution_kind",
            "#[serde(with = \"crate::serde_execution_kind\")]",
        )
        .compile(proto_files, &includes)
}

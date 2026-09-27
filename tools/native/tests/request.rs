//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Exercise action admission without acquiring privileged resources.

#![cfg(unix)]

use std::fs::File;
use std::io::Write;

use bsmr_native::request::{Error, Request};
use serde_json::{Value, json};

/// Use the executor's existing nested action schema.
fn wire() -> Value {
    json!({
        "environment": "runtime", "input": "0".repeat(64),
        "action": {"protocol": 1, "arguments": ["/bin/tool"], "environment": {},
                   "working_directory": "", "outputs": [{"path": "out/file", "kind": "file"}], "timeout_ms": 20}
    })
}

/// Leave the sender's position at EOF to verify positional reads.
fn file(value: &Value) -> File {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    file
}

#[test]
fn accepted_request_keeps_its_bound_runtime_and_deadline() {
    let request = Request::read(&file(&wire()), "runtime").unwrap();
    assert_eq!(request.timeout().as_millis(), 20);
    assert_eq!(request.action().arguments, ["/bin/tool"]);
    assert_eq!(request.input(), "0".repeat(64));
    assert!(matches!(
        Request::read(&file(&wire()), "changed"),
        Err(Error::Environment)
    ));
}

#[test]
fn invalid_commands_paths_and_overlaps_cannot_enter_the_worker() {
    for (pointer, value) in [
        ("/action/protocol", json!(2)),
        ("/action/working_directory", json!("../outside")),
        (
            "/action/outputs",
            json!([{"path":"/outside", "kind":"file"}]),
        ),
        (
            "/action/outputs",
            json!([{"path":"out", "kind":"directory"}, {"path":"out/file", "kind":"file"}]),
        ),
        ("/action/arguments", json!([])),
        ("/action/environment", json!({"INVALID=KEY":"value"})),
        (
            "/action/environment",
            json!({"BSMR_SCRATCH_PATH":"../outside"}),
        ),
        (
            "/action/environment",
            json!({"BSMR_SCRATCH_PATH":"out/file/nested"}),
        ),
        ("/action/arguments", json!(["tool\u{0}other"])),
        ("/action/timeout_ms", json!(u64::MAX)),
        ("/input", json!("not a digest")),
    ] {
        let mut wire = wire();
        *wire.pointer_mut(pointer).unwrap() = value;
        assert!(Request::read(&file(&wire), "runtime").is_err(), "{pointer}");
    }
}

#[test]
fn oversized_request_is_rejected_before_allocating_its_payload() {
    let source = tempfile::tempfile().unwrap();
    source.set_len(bsmr_sandbox::MAX_ACTION_BYTES + 1).unwrap();
    assert!(matches!(
        Request::read(&source, "runtime"),
        Err(Error::Size)
    ));
}

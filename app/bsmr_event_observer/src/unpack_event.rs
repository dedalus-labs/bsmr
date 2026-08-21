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

use bsmr_data::InstantEvent;
use bsmr_data::SpanEndEvent;
use bsmr_data::SpanStartEvent;
use bsmr_data::bsmr_event;
use bsmr_events::BsmrEvent;

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = InvalidEvent)]
pub enum VisitorError {
    #[error("Sent an event missing one or more fields: `{0:?}`")]
    MissingField(BsmrEvent),
    #[error("Sent an unexpected Record event: `{0:?}`")]
    UnexpectedRecord(BsmrEvent),
}

/// Just a simple structure that makes it easier to deal with BsmrEvent rather than
/// needing to deal with the unpacking of optional fields yourself.
pub enum UnpackedBsmrEvent<'a> {
    SpanStart(
        &'a BsmrEvent,
        &'a SpanStartEvent,
        &'a bsmr_data::span_start_event::Data,
    ),
    SpanEnd(
        &'a BsmrEvent,
        &'a SpanEndEvent,
        &'a bsmr_data::span_end_event::Data,
    ),
    Instant(
        &'a BsmrEvent,
        &'a InstantEvent,
        &'a bsmr_data::instant_event::Data,
    ),
    UnrecognizedSpanStart(&'a BsmrEvent, &'a SpanStartEvent),
    UnrecognizedSpanEnd(&'a BsmrEvent, &'a SpanEndEvent),
    UnrecognizedInstant(&'a BsmrEvent, &'a InstantEvent),
}

pub fn unpack_event(event: &BsmrEvent) -> bsmr_error::Result<UnpackedBsmrEvent<'_>> {
    match &event.data() {
        bsmr_event::Data::SpanStart(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedBsmrEvent::SpanStart(event, v, data)
            } else {
                UnpackedBsmrEvent::UnrecognizedSpanStart(event, v)
            }
        }),
        bsmr_event::Data::SpanEnd(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedBsmrEvent::SpanEnd(event, v, data)
            } else {
                UnpackedBsmrEvent::UnrecognizedSpanEnd(event, v)
            }
        }),
        bsmr_event::Data::Instant(v) => Ok({
            if let Some(data) = v.data.as_ref() {
                UnpackedBsmrEvent::Instant(event, v, data)
            } else {
                UnpackedBsmrEvent::UnrecognizedInstant(event, v)
            }
        }),
        bsmr_event::Data::Record(_) => Err(VisitorError::UnexpectedRecord(event.clone()).into()),
    }
}

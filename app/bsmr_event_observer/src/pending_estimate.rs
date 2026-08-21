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

use crate::dice_state::DiceState;
use crate::span_tracker::Roots;
use crate::span_tracker::SpanTrackable;

/// Estimate how many things are still left to do in a build. This is an approximation since our
/// roots and DICE state are not necessarily entirely in sync.
pub fn pending_estimate<T: SpanTrackable>(roots: &Roots<T>, dice: &DiceState) -> u64 {
    let mut total = 0;
    for k in &["BuildKey", "AnalysisKey"] {
        let from_dice = dice
            .key_states()
            .get(*k)
            .map_or(0, |v| v.started - v.finished);

        let from_roots = roots.dice_counts().get(k).copied().unwrap_or(0);

        total += u64::from(from_dice).saturating_sub(from_roots);
    }

    total
}

pub fn estimate_completion_percentage<T: SpanTrackable>(roots: &Roots<T>, dice: &DiceState) -> u8 {
    let from_dice = dice
        .key_states()
        .get("BuildKey")
        .map_or((0, 0), |v| (v.started, v.finished));

    let from_roots = roots.dice_counts().get("BuildKey").copied().unwrap_or(0);

    let started = u64::from(from_dice.0).saturating_sub(from_roots);
    let finished = u64::from(from_dice.1).saturating_sub(from_roots);
    if started == 0 {
        // Avoid divide by zero.
        return 0;
    }
    ((finished as f64 / started as f64) * 100f64) as u8
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::UNIX_EPOCH;

    use bsmr_data::SpanStartEvent;
    use bsmr_events::BsmrEvent;
    use bsmr_events::span::SpanId;
    use bsmr_hash::StdBsmrHashMap;
    use bsmr_wrapper_common::invocation_id::TraceId;

    use crate::dice_state::DiceState;
    use crate::pending_estimate::estimate_completion_percentage;
    use crate::span_tracker::BsmrEventSpanTracker;

    fn setup_roots(tracker: &mut BsmrEventSpanTracker) {
        let span = Arc::new(BsmrEvent::new(
            UNIX_EPOCH,
            TraceId::new(),
            Some(SpanId::next()),
            None,
            SpanStartEvent {
                data: Some(
                    bsmr_data::ActionExecutionStart {
                        key: Some(bsmr_data::ActionKey {
                            id: Default::default(),
                            owner: Some(bsmr_data::action_key::Owner::TargetLabel(
                                bsmr_data::ConfiguredTargetLabel {
                                    label: Some(bsmr_data::TargetLabel {
                                        package: "pkg".into(),
                                        name: "target".into(),
                                    }),
                                    configuration: Some(bsmr_data::Configuration {
                                        full_name: "conf".into(),
                                    }),
                                    execution_configuration: None,
                                },
                            )),
                            key: "".to_owned(),
                        }),
                        name: Some(bsmr_data::ActionName {
                            category: "category".into(),
                            identifier: "identifier".into(),
                        }),
                        kind: bsmr_data::ActionKind::NotSet as i32,
                    }
                    .into(),
                ),
            }
            .into(),
        ));
        tracker.start_at(&span).unwrap();
    }

    fn setup_dice_state(dice_state: &mut DiceState, finished: u32, total: u32) {
        dice_state.update(&bsmr_data::DiceStateSnapshot {
            key_states: {
                let mut map = StdBsmrHashMap::default();
                map.insert(
                    "BuildKey".to_owned(),
                    bsmr_data::DiceKeyState {
                        started: total,
                        finished,
                        check_deps_started: 0,
                        check_deps_finished: 0,
                        compute_started: 0,
                        compute_finished: 0,
                    },
                );
                map
            },
            core_state_queue_depth: 0,
        });
    }

    #[test]
    fn test_completion_no_progress() -> bsmr_error::Result<()> {
        let mut dice = DiceState::new();
        let mut tracker = BsmrEventSpanTracker::new();

        setup_roots(&mut tracker);
        setup_dice_state(&mut dice, 0, 100);
        assert_eq!(estimate_completion_percentage(tracker.roots(), &dice), 0);
        Ok(())
    }

    #[test]
    fn test_completion_percentage_build_complete() -> bsmr_error::Result<()> {
        let mut dice = DiceState::new();
        let mut tracker = BsmrEventSpanTracker::new();

        setup_roots(&mut tracker);
        setup_dice_state(&mut dice, 100, 100);
        assert_eq!(estimate_completion_percentage(tracker.roots(), &dice), 100);
        Ok(())
    }

    #[test]
    fn test_completion_percentage_intermediate_state() -> bsmr_error::Result<()> {
        let mut dice = DiceState::new();
        let mut tracker = BsmrEventSpanTracker::new();

        setup_roots(&mut tracker);
        // 26/101 -> 25/100 since we have 1 subtracted for the ActionExecutionStart
        setup_dice_state(&mut dice, 26, 101);
        assert_eq!(estimate_completion_percentage(tracker.roots(), &dice), 25);
        Ok(())
    }

    #[test]
    fn test_completion_percentage_invalid_dice_state() -> bsmr_error::Result<()> {
        let mut dice = DiceState::new();
        let mut tracker = BsmrEventSpanTracker::new();

        setup_roots(&mut tracker);
        setup_dice_state(&mut dice, 10, 0);
        assert_eq!(estimate_completion_percentage(tracker.roots(), &dice), 0);
        Ok(())
    }

    #[test]
    fn test_completion_percentage_empty_span() -> bsmr_error::Result<()> {
        let mut dice = DiceState::new();
        let tracker = BsmrEventSpanTracker::new();

        setup_dice_state(&mut dice, 26, 101);
        assert_eq!(estimate_completion_percentage(tracker.roots(), &dice), 25);
        Ok(())
    }
}

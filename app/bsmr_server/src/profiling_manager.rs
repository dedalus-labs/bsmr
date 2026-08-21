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

use std::sync::Arc;

use bsmr_cli_proto::client_context::ProfilePatternOptions;
use bsmr_error::BsmrErrorContext;
use bsmr_events::dispatch::EventDispatcher;
use bsmr_fs::paths::abs_path::AbsPathBuf;
use bsmr_interpreter::starlark_profiler::config::ProfileRegex;
use bsmr_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use bsmr_profile::proto_to_profile_mode;

use crate::profile_patterns::FileWritingProfileEventListener;

#[derive(bsmr_error::Error)]
pub enum BsmrdServerError {
    #[error(
        "server received multiple profiler configurations. this is due to using both --profile-patterns and one of the command specific profiling args"
    )]
    #[bsmr(tag=bsmr_error::ErrorTag::Input)]
    StarlarkProfilerConfigurationConflict,
}

pub struct StarlarkProfilingManager {
    pub(crate) configuration: StarlarkProfilerConfiguration,
    pub(crate) profile_event_listener: Option<Arc<FileWritingProfileEventListener>>,
}

impl StarlarkProfilingManager {
    pub fn new(
        profile_pattern_opts: Option<&ProfilePatternOptions>,
        starlark_profile_override: StarlarkProfilerConfiguration,
        events: &EventDispatcher,
    ) -> bsmr_error::Result<Self> {
        let starlark_profile_patterns_opts = match profile_pattern_opts {
            Some(ProfilePatternOptions {
                profile_patterns,
                profile_mode,
                profile_output,
            }) => Some(StarlarkProfilerConfiguration::ProfilePattern(
                proto_to_profile_mode(
                    (*profile_mode)
                        .try_into()
                        .internal_error("invalid profile mode enum value")?,
                ),
                ProfileRegex::new(profile_patterns)?,
                AbsPathBuf::try_from(profile_output.to_owned())?,
            )),
            None => None,
        };
        let configuration = match (starlark_profile_patterns_opts, starlark_profile_override) {
            (None, v) => v,
            (Some(v), StarlarkProfilerConfiguration::None) => v,
            (Some(_), _) => Err(BsmrdServerError::StarlarkProfilerConfigurationConflict)?,
        };

        let profile_event_listener = if let StarlarkProfilerConfiguration::ProfilePattern(
            _,
            _,
            output_path,
        ) = &configuration
        {
            Some(Arc::new(FileWritingProfileEventListener::new(
                output_path.join(format!(
                    "{}-{}",
                    chrono::Local::now().format("%Y-%m-%d-%H-%M"),
                    events.trace_id()
                )),
            )))
        } else {
            None
        };

        Ok(Self {
            configuration,
            profile_event_listener,
        })
    }

    pub fn finalize(&self) -> bsmr_error::Result<()> {
        if let Some(v) = &self.profile_event_listener {
            v.finalize()?;
        }
        Ok(())
    }
}

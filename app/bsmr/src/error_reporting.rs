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

use bsmr_error::BsmrErrorContext;

/// Initializes structured soft-error reporting.
pub fn initialize() -> bsmr_error::Result<()> {
    bsmr_core::error::initialize(Box::new(move |category, err, loc, options| {
        imp::write_soft_error(
            category,
            err,
            bsmr_data::Location {
                file: loc.0.to_owned(),
                line: loc.1,
                column: loc.2,
            },
            options,
        );
    }))
    .bsmr_error_context("Error initializing soft errors")?;
    Ok(())
}

mod imp {
    use bsmr_core::error::StructuredErrorOptions;
    use bsmr_data::Location;
    use bsmr_events::daemon_id::get_daemon_id_for_panics;
    use bsmr_events::metadata;
    use bsmr_hash::StdBsmrHashMap;

    fn get_metadata(options: &StructuredErrorOptions) -> StdBsmrHashMap<String, String> {
        #[cfg_attr(client_only, allow(unused_mut))]
        let mut map = metadata::collect(&get_daemon_id_for_panics());
        #[cfg(not(client_only))]
        if let Some(commands) = bsmr_server::active_commands::try_active_commands() {
            let commands = commands.keys().map(|id| id.to_string()).collect::<Vec<_>>();
            map.insert("active_commands".to_owned(), commands.join(","));
        }
        if let Some(logview_key) = options
            .low_cardinality_key_for_additional_logview_samples
            .as_ref()
        {
            map.insert(
                "low_cardinality_key_for_additional_logview_samples".to_owned(),
                logview_key.to_string(),
            );
        }
        map
    }

    pub(crate) fn write_soft_error(
        category: &str,
        err: &bsmr_error::Error,
        location: Location,
        options: StructuredErrorOptions,
    ) {
        let event = structured_error_payload(
            Some(location),
            format!("Soft Error: {category}: {err:#}"),
            &options,
            Some(bsmr_data::SoftError {
                category: category.to_owned(),
                is_quiet: options.quiet,
            }),
        );

        // If the soft error was fired in a context with an ambient dispatcher, then we only send
        // it there, but some contexts don't have one, and in that case, we notify all running
        // commands.
        match bsmr_events::dispatch::get_dispatcher_opt() {
            Some(dispatcher) => {
                dispatcher.instant_event(event.clone());
            }
            None => {
                #[cfg(client_only)]
                let warn = !options.quiet;
                #[cfg(not(client_only))]
                let warn = !bsmr_server::active_commands::broadcast_instant_event(&event)
                    && !options.quiet;
                if warn {
                    tracing::warn!("Warning \"{}\": {:#}", category, err);
                }
            }
        }
    }

    fn structured_error_payload(
        location: Option<Location>,
        message: String,
        options: &StructuredErrorOptions,
        soft_error_category: Option<bsmr_data::SoftError>,
    ) -> bsmr_data::StructuredError {
        let metadata = get_metadata(options);
        bsmr_data::StructuredError {
            location,
            payload: message,
            metadata,
            backtrace: Vec::new(),
            quiet: options.quiet,
            task: Some(options.task),
            soft_error_category: soft_error_category
                .map(|arg0: bsmr_data::SoftError| ToOwned::to_owned(&arg0)),
            daemon_in_memory_state_is_corrupted: options.daemon_in_memory_state_is_corrupted,
            daemon_materializer_state_is_corrupted: options.daemon_materializer_state_is_corrupted,
            action_cache_is_corrupted: options.action_cache_is_corrupted,
            deprecation: options.deprecation,
        }
    }
}

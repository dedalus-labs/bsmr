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
use std::sync::Mutex;

use bsmr_error::internal_error;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_path::AbsPathBuf;
use bsmr_fs::paths::file_name::FileName;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use bsmr_hash::StdBsmrHashMap;
use bsmr_interpreter::dice::starlark_provider::StarlarkEvalKind;
use bsmr_interpreter::factory::ProfileEventListener;
use bsmr_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use bsmr_profile::write_starlark_flamegraph;
use dupe::Dupe;
use itertools::Itertools;

pub(crate) struct FileWritingProfileEventListener {
    base_path: AbsPathBuf,
    state: Mutex<State>,
}

struct State {
    written: StdBsmrHashMap<ForwardRelativePathBuf, usize>,
    errors: Vec<bsmr_error::Error>,
    profiles: Vec<Arc<StarlarkProfileDataAndStats>>,
}

impl FileWritingProfileEventListener {
    pub(crate) fn new(base_path: AbsPathBuf) -> Self {
        Self {
            base_path,
            state: Mutex::new(State {
                written: StdBsmrHashMap::default(),
                errors: Vec::new(),
                profiles: Vec::new(),
            }),
        }
    }
}

impl FileWritingProfileEventListener {
    /// Writes the all_keys.list file and returns the path to the merged SVG if one was generated.
    pub fn finalize(&self) -> bsmr_error::Result<Option<AbsPathBuf>> {
        let lock = self.state.lock().unwrap();
        fs_util::create_dir_all(&self.base_path)?;
        let merged_profile =
            StarlarkProfileDataAndStats::merge(lock.profiles.iter().map(|p| p.as_ref()))?;

        fs_util::write(
            self.base_path.join("all_keys.list"),
            merged_profile.targets.iter().join("\n"),
        )
        .categorize_internal()?;
        let merged_prefix = self.base_path.join("merged");
        write_profile_data(&merged_profile, merged_prefix.clone())?;

        if let Some(e) = lock.errors.first() {
            return Err(e.dupe());
        }

        let merged_svg = merged_prefix.with_added_extension("svg");
        if merged_svg.exists() {
            Ok(Some(merged_svg))
        } else {
            Ok(None)
        }
    }

    fn handle_profile_collected(
        &self,
        eval_kind: StarlarkEvalKind,
        profile_data: &Arc<StarlarkProfileDataAndStats>,
    ) -> bsmr_error::Result<()> {
        let subpath = eval_kind.as_path()?;

        let suffix = {
            let mut lock = self.state.lock().unwrap();
            lock.profiles.push(profile_data.dupe());
            match lock.written.entry(subpath.clone()) {
                std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                    *occupied_entry.get_mut() += 1;
                    format!("-{}", *occupied_entry.get())
                }
                std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(1);
                    "".to_owned()
                }
            }
        };

        let subpath = subpath
            .parent()
            .ok_or_else(|| internal_error!("profiling path has no parent"))?
            .join(FileName::new(&format!(
                "{}{}",
                subpath
                    .file_name()
                    .ok_or_else(|| internal_error!("profiling path has no filename"))?,
                suffix,
            ))?);

        let output_path = self.base_path.join(subpath.as_path());
        write_profile_data(profile_data, output_path)?;
        Ok(())
    }
}

fn write_profile_data(
    profile_data: &StarlarkProfileDataAndStats,
    output_path_prefix: AbsPathBuf,
) -> Result<(), bsmr_error::Error> {
    fs_util::create_dir_all(output_path_prefix.parent().unwrap())?;
    fs_util::write(
        output_path_prefix.with_added_extension("profile"),
        profile_data.profile_data.gen_csv()?,
    )
    .categorize_internal()?;

    if let Some(flame_profile) = profile_data.profile_data.gen_flame_data()? {
        write_starlark_flamegraph(
            flame_profile,
            &output_path_prefix,
            inferno::flamegraph::Options::default(),
        )?;
    }
    Ok(())
}

impl ProfileEventListener for FileWritingProfileEventListener {
    fn profile_collected(
        &self,
        eval_kind: StarlarkEvalKind,
        profile_data: &Arc<StarlarkProfileDataAndStats>,
    ) {
        if let Err(e) = self.handle_profile_collected(eval_kind, profile_data) {
            let mut lock = self.state.lock().unwrap();
            lock.errors.push(e);
        }
    }
}

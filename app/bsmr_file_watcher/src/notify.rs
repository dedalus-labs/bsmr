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

use std::mem;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::time::Duration;

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_common::file_ops::dice::FileChangeTracker;
use bsmr_common::ignores::ignore_set::IgnoreSet;
use bsmr_common::invocation_paths::InvocationPaths;
use bsmr_core::cells::CellResolver;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::name::CellName;
use bsmr_core::fs::project::ProjectRoot;
use bsmr_data::FileWatcherEventType;
use bsmr_data::FileWatcherKind;
use bsmr_error::conversion::from_any_with_tag;
use bsmr_events::dispatch::span_async;
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_hash::StdBuckHashMap;
use dice::DiceTransactionUpdater;
use dupe::Dupe;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::CreateKind;
use notify::event::MetadataKind;
use notify::event::ModifyKind;
use notify::event::RemoveKind;
use starlark_map::ordered_set::OrderedSet;
use tracing::debug;
use tracing::info;
use uuid::Uuid;

use crate::file_watcher::FileWatcher;
use crate::mergebase::Mergebase;
use crate::stats::FileWatcherStats;

fn ignore_event_kind(event_kind: EventKind) -> bool {
    match event_kind {
        EventKind::Access(_) => true,
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::Ownership))
        | EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)) => false,
        EventKind::Modify(ModifyKind::Metadata(_)) => true,
        _ => false,
    }
}

/// Buffer containing the events that have happened since we last got a message.
/// Used to dedupe events, since notify sends a notification on every change.
#[derive(Allocative)]
struct NotifyFileData {
    ignored: u64,
    #[allocative(skip)]
    events: OrderedSet<(CellPath, EventKind)>,
    /// Whether file system changes were missed
    missed_events: bool,
}

impl NotifyFileData {
    fn new() -> Self {
        Self {
            ignored: 0,
            events: OrderedSet::new(),
            missed_events: false,
        }
    }

    fn process(
        &mut self,
        event: notify::Result<notify::Event>,
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdBuckHashMap<CellName, IgnoreSet>,
    ) -> bsmr_error::Result<()> {
        let event = event.map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::NotifyWatcher))?;

        for path in &event.paths {
            // Testing shows that we get absolute paths back from the `notify` library.
            // It's not documented though.
            let path = root.relativize(AbsNormPath::new(&path)?)?;

            // We ignore the buck-out prefix, as those are uninteresting events caused by us.
            // We also ignore other buck-out directories, as if you have two isolation dirs running at once, they are not interesting.
            // We do this in the notify-watcher, rather than a generic layer, as watchman users should configure
            // to ignore buck-out, to reduce the number of events, rather than hiding them later.
            if path.starts_with(InvocationPaths::buck_out_dir_prefix()) {
                // We don't want to event add them as ignored events, since they are super common
                // and very boring
                continue;
            }

            let cell_path = cells.get_cell_path(&path);
            let ignore = ignore_specs
                .get(&cell_path.cell())
                // See the comment on the analogous code in `watchman/interface.rs`
                .is_some_and(|ignore| ignore.is_match(cell_path.path()));

            info!(
                "FileWatcher: {:?} {:?} (ignore = {})",
                path, &event.kind, ignore
            );

            if event.need_rescan() {
                self.missed_events = true;
                debug!("FileWatcher: File change events were missed");
            }

            if ignore || ignore_event_kind(event.kind) {
                self.ignored += 1;
            } else {
                self.events.insert((cell_path, event.kind));
            }
        }
        Ok(())
    }

    fn sync(self) -> (bsmr_data::FileWatcherStats, Option<FileChangeTracker>) {
        // The changes that go into the DICE transaction
        let mut changed = FileChangeTracker::new();
        // If we missed events, sync2() will drop the entire DICE graph. Surface that to
        // telemetry/UI by reusing the fresh-instance fields the watchman path uses for
        // the equivalent wipe.
        let base = if self.missed_events {
            bsmr_data::FileWatcherStats {
                fresh_instance: true,
                fresh_instance_data: Some(bsmr_data::FreshInstance {
                    new_mergebase: false,
                    cleared_dice: true,
                    cleared_dep_files: false,
                }),
                incomplete_events_reason: Some(
                    "notify dropped events (kernel queue overflow)".to_owned(),
                ),
                ..Default::default()
            }
        } else {
            Default::default()
        };
        let mut stats = FileWatcherStats::new(base, self.events.len());
        stats.add_ignored(self.ignored);

        for (cell_path, event_kind) in self.events {
            let cell_path_str = cell_path.to_string();
            match event_kind {
                EventKind::Create(create_kind) => match create_kind {
                    CreateKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                    }
                    CreateKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                    CreateKind::Any | CreateKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Modify(modify_kind) => match modify_kind {
                    ModifyKind::Data(_) | ModifyKind::Metadata(_) => {
                        changed.file_contents_changed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Modify,
                            FileWatcherKind::File,
                        );
                    }
                    ModifyKind::Name(_) | ModifyKind::Any | ModifyKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Remove(remove_kind) => match remove_kind {
                    RemoveKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                    }
                    RemoveKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                    RemoveKind::Any | RemoveKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                _ => {}
            }
        }

        let stats = stats.finish();
        let changed = if self.missed_events {
            None
        } else {
            Some(changed)
        };

        (stats, changed)
    }
}

#[derive(Default)]
struct NotifyEventBarrier {
    expected: Option<PathBuf>,
    observed: bool,
}

#[derive(Allocative)]
pub struct NotifyFileWatcher {
    #[allocative(skip)]
    #[expect(unused)]
    // FIXME(JakobDegen): Clarify if this just needs to be kept alive or can be removed?
    watcher: RecommendedWatcher,
    data: Arc<Mutex<bsmr_error::Result<NotifyFileData>>>,
    #[allocative(skip)]
    barrier: Arc<(Mutex<NotifyEventBarrier>, Condvar)>,
    #[allocative(skip)]
    barrier_dir: PathBuf,
    #[allocative(skip)]
    sync_lock: Mutex<()>,
}

impl NotifyFileWatcher {
    pub fn new(
        root: &ProjectRoot,
        cells: CellResolver,
        ignore_specs: StdBuckHashMap<CellName, IgnoreSet>,
    ) -> bsmr_error::Result<Self> {
        let data = Arc::new(Mutex::new(Ok(NotifyFileData::new())));
        let data2 = data.dupe();
        let root2 = root.dupe();
        let barrier = Arc::new((Mutex::new(NotifyEventBarrier::default()), Condvar::new()));
        let barrier2 = barrier.dupe();
        let barrier_dir = root
            .root()
            .as_path()
            .join(InvocationPaths::buck_out_dir_prefix().as_str());
        std::fs::create_dir_all(&barrier_dir)
            .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::NotifyWatcher))?;
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let observed_barrier = event.as_ref().ok().and_then(|event| {
                    let barrier = barrier2.0.lock().unwrap();
                    barrier
                        .expected
                        .as_ref()
                        .filter(|expected| event.paths.iter().any(|path| path == *expected))
                        .cloned()
                });
                let mut guard = data2.lock().unwrap();
                if let Ok(state) = &mut *guard {
                    if let Err(e) = state.process(event, &root2, &cells, &ignore_specs) {
                        *guard = Err(e);
                    }
                }
                drop(guard);

                if let Some(observed_barrier) = observed_barrier {
                    let mut barrier = barrier2.0.lock().unwrap();
                    if barrier.expected.as_ref() == Some(&observed_barrier) {
                        barrier.observed = true;
                        barrier2.1.notify_all();
                    }
                }
            })
            .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::NotifyWatcher))?;
        watcher
            .watch(root.root().as_path(), notify::RecursiveMode::Recursive)
            .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::NotifyWatcher))?;
        Ok(Self {
            watcher,
            data,
            barrier,
            barrier_dir,
            sync_lock: Mutex::new(()),
        })
    }

    /// Waits until the watcher callback has processed every event queued before this call.
    fn synchronize_events(&self) -> bsmr_error::Result<()> {
        let marker = self
            .barrier_dir
            .join(format!(".bsmr-notify-barrier-{}", Uuid::new_v4()));
        {
            let mut barrier = self.barrier.0.lock().unwrap();
            barrier.expected = Some(marker.clone());
            barrier.observed = false;
        }

        if let Err(error) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            self.barrier.0.lock().unwrap().expected = None;
            return Err(from_any_with_tag(
                error,
                bsmr_error::ErrorTag::NotifyWatcher,
            ));
        }

        let barrier = self.barrier.0.lock().unwrap();
        let (mut barrier, timeout) = self
            .barrier
            .1
            .wait_timeout_while(barrier, Duration::from_secs(10), |state| !state.observed)
            .unwrap();
        let observed = barrier.observed;
        barrier.expected = None;
        barrier.observed = false;
        drop(barrier);

        let remove_result = std::fs::remove_file(&marker)
            .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::NotifyWatcher));
        if timeout.timed_out() && !observed {
            return Err(from_any_with_tag(
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "notify did not observe its synchronization marker `{}`",
                        marker.display()
                    ),
                ),
                bsmr_error::ErrorTag::NotifyWatcher,
            ));
        }
        remove_result
    }

    fn sync2(
        &self,
        mut dice: DiceTransactionUpdater,
    ) -> bsmr_error::Result<(bsmr_data::FileWatcherStats, DiceTransactionUpdater)> {
        let _sync = self.sync_lock.lock().unwrap();
        self.synchronize_events()?;
        let old = {
            let mut guard = self.data.lock().unwrap();
            mem::replace(&mut *guard, Ok(NotifyFileData::new()))
        };
        let (stats, changes) = old?.sync();
        if let Some(changes) = changes {
            changes.write_to_dice(&mut dice)?;
        } else {
            // We missed some file system notifications, so we drop everything
            dice = dice.unstable_take();
        }
        Ok((stats, dice))
    }
}

#[async_trait]
impl FileWatcher for NotifyFileWatcher {
    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> bsmr_error::Result<(DiceTransactionUpdater, Mergebase)> {
        span_async(
            bsmr_data::FileWatcherStart {
                provider: bsmr_data::FileWatcherProvider::RustNotify as i32,
            },
            async {
                let (stats, res) = match self.sync2(dice) {
                    Ok((stats, dice)) => {
                        let mergebase = Mergebase(Arc::new(stats.branched_from_revision.clone()));
                        ((Some(stats)), Ok((dice, mergebase)))
                    }
                    Err(e) => (None, Err(e)),
                };
                (res, bsmr_data::FileWatcherEnd { stats })
            },
        )
        .await
    }
}

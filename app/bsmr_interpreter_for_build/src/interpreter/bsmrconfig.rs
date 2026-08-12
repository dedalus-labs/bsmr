//===----------------------------------------------------------------------===//
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

use std::cell::RefCell;
use std::fmt;
use std::ops::DerefMut;
use std::sync::Arc;

use bsmr_common::legacy_configs::configs::LegacyBsmrConfig;
use bsmr_common::legacy_configs::dice::OpaqueLegacyBsmrConfigOnDice;
use bsmr_common::legacy_configs::key::BsmrconfigKeyRef;
use bsmr_core::soft_error;
use dice::DiceComputations;
use hashbrown::HashTable;
use starlark::collections::Hashed;
use starlark::eval::Evaluator;
use starlark::values::FrozenStringValue;
use starlark::values::StringValue;

struct BsmrConfigEntry {
    section: Hashed<String>,
    key: Hashed<String>,
    value: Option<FrozenStringValue>,
}

pub trait BsmrConfigsViewForStarlark {
    fn read_current_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>>;

    fn read_root_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>>;
}

struct BsmrConfigsInner<'a> {
    configs_view: &'a mut (dyn BsmrConfigsViewForStarlark + 'a),
    /// Hash map by `(section, key)` pair, so we do one table lookup per request.
    /// So we hash the `key` even if the section does not exist,
    /// but this is practically not an issue, because keys usually come with cached hash.
    current_cell_cache: HashTable<BsmrConfigEntry>,
    root_cell_cache: HashTable<BsmrConfigEntry>,
}

/// Version of cell bsmrconfig optimized for fast query from `read_config` Starlark function.
pub(crate) struct LegacyBsmrConfigsForStarlark<'a> {
    inner: RefCell<BsmrConfigsInner<'a>>,
}

impl<'a> fmt::Debug for LegacyBsmrConfigsForStarlark<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyBsmrConfigForStarlark")
            .finish_non_exhaustive()
    }
}

impl<'a> LegacyBsmrConfigsForStarlark<'a> {
    // `section` or `key` 32 bit hashes are well swizzled,
    // but concatenation of them into 64 bit integer is not.
    // This function tries to fix that.
    fn mix_hashes(a: u32, b: u32) -> u64 {
        fn murmur3_mix64(mut x: u64) -> u64 {
            x ^= x >> 33;
            x = x.wrapping_mul(0xff51afd7ed558ccd);
            x ^= x >> 33;
            x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
            x ^= x >> 33;
            x
        }

        murmur3_mix64(((a as u64) << 32) | (b as u64))
    }

    /// Constructor.
    pub(crate) fn new(
        configs_view: &'a mut (dyn BsmrConfigsViewForStarlark + 'a),
    ) -> LegacyBsmrConfigsForStarlark<'a> {
        LegacyBsmrConfigsForStarlark {
            inner: RefCell::new(BsmrConfigsInner {
                configs_view,
                current_cell_cache: HashTable::new(),
                root_cell_cache: HashTable::new(),
            }),
        }
    }

    fn get_impl(
        &self,
        section: Hashed<&str>,
        key: Hashed<&str>,
        from_root_cell: bool,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> bsmr_error::Result<Option<FrozenStringValue>> {
        let hash = Self::mix_hashes(section.hash().get(), key.hash().get());

        let mut inner = self.inner.borrow_mut();
        let BsmrConfigsInner {
            configs_view,
            current_cell_cache,
            root_cell_cache,
        } = inner.deref_mut();

        let cache = if from_root_cell {
            root_cell_cache
        } else {
            current_cell_cache
        };
        if let Some(e) = cache.find(hash, |e| {
            e.section.key() == section.key() && e.key.as_str() == *key.key()
        }) {
            return Ok(e.value);
        }

        let value = if from_root_cell {
            configs_view.read_root_cell_config(BsmrconfigKeyRef {
                section: section.key(),
                property: key.key(),
            })?
        } else {
            configs_view.read_current_cell_config(BsmrconfigKeyRef {
                section: section.key(),
                property: key.key(),
            })?
        }
        .map(|v| eval.frozen_heap().alloc_str(&v));

        cache.insert_unique(
            hash,
            BsmrConfigEntry {
                section: Hashed::new_unchecked(section.hash(), (*section.key()).to_owned()),
                key: Hashed::new_unchecked(key.hash(), (*key.key()).to_owned()),
                value,
            },
            |e| Self::mix_hashes(e.section.hash().get(), e.key.hash().get()),
        );

        Ok(value)
    }

    /// Find the bsmrconfig entry.
    pub(crate) fn current_cell_get(
        &self,
        section: StringValue,
        key: StringValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> bsmr_error::Result<Option<FrozenStringValue>> {
        // Note here we reuse the hashes of `section` and `key`,
        // if `read_config` is called repeatedly with the same constant arguments:
        // `StringValue` caches the hashes.
        self.get_impl(section.get_hashed_str(), key.get_hashed_str(), false, eval)
    }

    pub(crate) fn root_cell_get(
        &self,
        section: StringValue,
        key: StringValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> bsmr_error::Result<Option<FrozenStringValue>> {
        // Note here we reuse the hashes of `section` and `key`,
        // if `read_config` is called repeatedly with the same constant arguments:
        // `StringValue` caches the hashes.
        self.get_impl(section.get_hashed_str(), key.get_hashed_str(), true, eval)
    }
}

pub(crate) struct ConfigsOnDiceViewForStarlark<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    bsmrconfig: OpaqueLegacyBsmrConfigOnDice,
    root_bsmrconfig: OpaqueLegacyBsmrConfigOnDice,
}

impl<'a, 'd> ConfigsOnDiceViewForStarlark<'a, 'd> {
    pub(crate) fn new(
        ctx: &'a mut DiceComputations<'d>,
        bsmrconfig: OpaqueLegacyBsmrConfigOnDice,
        root_bsmrconfig: OpaqueLegacyBsmrConfigOnDice,
    ) -> Self {
        Self {
            ctx,
            bsmrconfig,
            root_bsmrconfig,
        }
    }
}

impl BsmrConfigsViewForStarlark for ConfigsOnDiceViewForStarlark<'_, '_> {
    fn read_current_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        read_config_and_report_deprecated(self.ctx, &self.bsmrconfig, key)
    }

    fn read_root_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        read_config_and_report_deprecated(self.ctx, &self.root_bsmrconfig, key)
    }
}

#[derive(Debug, bsmr_error::Error)]
#[error("{} is no longer used. {}", .0, .1)]
#[bsmr(tag = Input)]
struct DeprecatedConfigError(String, Arc<str>);

fn read_config_and_report_deprecated(
    ctx: &mut DiceComputations,
    config: &OpaqueLegacyBsmrConfigOnDice,
    key: BsmrconfigKeyRef,
) -> bsmr_error::Result<Option<Arc<str>>> {
    let result = config.lookup(ctx, key)?;
    let property = format!("{}.{}", key.section, key.property);

    let key = BsmrconfigKeyRef {
        section: "deprecated_config",
        property: &property,
    };
    let msg = config.lookup(ctx, key)?;
    if let Some(msg) = msg {
        // soft error category can only contain ascii lowercese characters
        let section = transform_logview_category(key.section);
        let prop = transform_logview_category(key.property);

        soft_error!(
            format!("deprecated_config_{section}_{prop}").as_str(),
            DeprecatedConfigError(property, msg).into(),
            quiet: true,
            error_on_oss: true
        )?;
    }
    Ok(result)
}

fn transform_logview_category(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_lowercase() || *c == '_')
        .collect::<String>()
}

pub struct LegacyConfigsViewForStarlark {
    current_cell_config: LegacyBsmrConfig,
    root_cell_config: LegacyBsmrConfig,
}

impl LegacyConfigsViewForStarlark {
    pub(crate) fn new(bsmrconfig: LegacyBsmrConfig, root_bsmrconfig: LegacyBsmrConfig) -> Self {
        Self {
            current_cell_config: bsmrconfig,
            root_cell_config: root_bsmrconfig,
        }
    }
}

impl BsmrConfigsViewForStarlark for LegacyConfigsViewForStarlark {
    fn read_current_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        Ok(self
            .current_cell_config
            .get(key)
            .map(|v| v.to_owned().into()))
    }

    fn read_root_cell_config(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        Ok(self.root_cell_config.get(key).map(|v| v.to_owned().into()))
    }
}

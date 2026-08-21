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

//! Dice operations for legacy configuration

use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_core::cells::name::CellName;
use bsmr_error::BsmrErrorContext;
use bsmr_error::internal_error;
use bsmr_events::dispatch::get_dispatcher;
use derive_more::Display;
use dice::DiceComputations;
use dice::DiceProjectionComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::OpaqueValue;
use dice::PagableValueSerialize;
use dice::ProjectionKey;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;

use crate::dice::cells::HasCellResolver;
use crate::legacy_configs::cells::BsmrConfigBasedCells;
use crate::legacy_configs::cells::ExternalBsmrconfigData;
use crate::legacy_configs::configs::LegacyBsmrConfig;
use crate::legacy_configs::key::BsmrconfigKeyRef;
use crate::legacy_configs::view::LegacyBsmrConfigView;

/// Bsmrconfig view which queries bsmrconfig entry from DICE.
#[derive(Clone, Dupe)]
pub struct OpaqueLegacyBsmrConfigOnDice {
    config: Arc<OpaqueValue<LegacyBsmrConfigForCellKey>>,
}

impl std::fmt::Debug for OpaqueLegacyBsmrConfigOnDice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyBsmrConfigOnDice")
            .field("config", &self.config)
            .finish()
    }
}

impl OpaqueLegacyBsmrConfigOnDice {
    pub fn lookup(
        &self,
        ctx: &mut DiceComputations,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        let BsmrconfigKeyRef { section, property } = key;
        Ok(ctx.projection(
            &*self.config,
            &LegacyBsmrConfigPropertyProjectionKey {
                section: section.to_owned(),
                property: property.to_owned(),
            },
        )?)
    }

    pub fn view<'a, 'd>(
        &'a self,
        ctx: &'a mut DiceComputations<'d>,
    ) -> LegacyBsmrConfigOnDice<'a, 'd> {
        LegacyBsmrConfigOnDice { ctx, config: self }
    }
}

pub struct LegacyBsmrConfigOnDice<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    config: &'a OpaqueLegacyBsmrConfigOnDice,
}

impl LegacyBsmrConfigOnDice<'_, '_> {
    pub fn parse<T: FromStr>(&mut self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyBsmrConfig::parse_value(key, self.get(key)?.as_deref())
    }
}

impl std::fmt::Debug for LegacyBsmrConfigOnDice<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyBsmrConfigOnDice")
            .field("config", &self.config)
            .finish()
    }
}

impl LegacyBsmrConfigView for LegacyBsmrConfigOnDice<'_, '_> {
    fn get(&mut self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<Arc<str>>> {
        self.config.lookup(self.ctx, key)
    }
}

pub trait HasInjectedLegacyConfigs {
    fn get_injected_external_bsmrconfig_data(
        &mut self,
    ) -> impl Future<Output = bsmr_error::Result<Arc<ExternalBsmrconfigData>>>;

    fn is_injected_external_bsmrconfig_data_key_set(
        &mut self,
    ) -> impl Future<Output = bsmr_error::Result<bool>>;
}

#[async_trait]
pub trait HasLegacyConfigs {
    /// Get bsmrconfigs.
    ///
    /// This operation does not record bsmrconfig as a dependency of current computation.
    /// Accessing specific bsmrconfig property, records that key as dependency.
    async fn get_legacy_config_on_dice(
        &mut self,
        cell_name: CellName,
    ) -> bsmr_error::Result<OpaqueLegacyBsmrConfigOnDice>;

    async fn get_legacy_root_config_on_dice(
        &mut self,
    ) -> bsmr_error::Result<OpaqueLegacyBsmrConfigOnDice>;

    /// Use this function carefully: a computation which fetches this key will be recomputed
    /// if any bsmrconfig property changes.
    ///
    /// Consider using `get_legacy_config_property` instead.
    async fn get_legacy_config_for_cell(
        &mut self,
        cell_name: CellName,
    ) -> bsmr_error::Result<LegacyBsmrConfig>;

    async fn get_legacy_config_property(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<Arc<str>>>;

    async fn parse_legacy_config_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static;

    async fn parse_legacy_config_list_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<Vec<T>>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static;
}

pub trait SetLegacyConfigs {
    fn set_legacy_config_external_data(
        &mut self,
        overrides: ExternalBsmrconfigData,
    ) -> bsmr_error::Result<()>;

    fn set_none_legacy_config_external_data(&mut self) -> bsmr_error::Result<()>;
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
#[display("{:?}", self)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct LegacyExternalBsmrConfigDataKey;

impl InjectedKey for LegacyExternalBsmrConfigDataKey {
    type Value = Option<Arc<ExternalBsmrconfigData>>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

#[derive(Clone, Display, Debug, Hash, Eq, PartialEq, Allocative, Pagable)]
#[display("LegacyBsmrConfigForCellKey({})", self.cell_name)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct LegacyBsmrConfigForCellKey {
    cell_name: CellName,
}

#[async_trait]
impl Key for LegacyBsmrConfigForCellKey {
    type Value = bsmr_error::Result<LegacyBsmrConfig>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> bsmr_error::Result<LegacyBsmrConfig> {
        let cells = ctx.get_cell_resolver().await?;
        let this_cell = cells.get(self.cell_name)?;
        let config = BsmrConfigBasedCells::parse_single_cell_with_dice(ctx, this_cell.path())
            .await
            .with_bsmr_error_context(|| {
                format!("Computing legacy bsmrconfigs for cell `{}`", self.cell_name)
            })?;
        let config = config.filter_values(is_config_invisible_to_dice);

        let event = bsmr_data::CellHasNewConfigs {
            cell: self.cell_name.as_str().to_owned(),
        };
        get_dispatcher().instant_event(event);

        Ok(config)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.compare(y),
            _ => false,
        }
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

/// The computation `LegacyBsmrConfigForCellKey` computation might encounter an error.
///
/// We can't return that error immediately, because we only compute the opaque value. We could
/// return the error when doing the projection to the bsmrconfig values, but that would result in us
/// increasing the size of the value returned from that computation. Instead, we'll use a different
/// projection key to extract just the error from the cell computation, and compute that when
/// constructing the `OpaqueLegacyBsmrConfigOnDice`.
#[derive(Debug, Display, Hash, Eq, PartialEq, Clone, Allocative, Pagable)]
#[pagable_typetag(dice::DiceProjectionDyn)]
struct LegacyBsmrConfigErrorKey();

impl ProjectionKey for LegacyBsmrConfigErrorKey {
    type DeriveFromKey = LegacyBsmrConfigForCellKey;
    type Value = Option<bsmr_error::Error>;

    fn compute(
        &self,
        config: &bsmr_error::Result<LegacyBsmrConfig>,
        _ctx: &DiceProjectionComputations,
    ) -> Option<bsmr_error::Error> {
        config.as_ref().err().cloned()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.is_none() && y.is_none()
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        struct T;
        impl ValueSerialize for T {
            type Value = <LegacyBsmrConfigErrorKey as ProjectionKey>::Value;

            fn pagable_serialize_value(
                &self,
                v: &Self::Value,
                _ser: &mut dyn pagable::PagableSerializer,
            ) -> Option<pagable::Result<()>> {
                match v {
                    Some(_) => unimplemented!(),
                    None => Some(Ok(())),
                }
            }

            fn pagable_deserialize_value<'de, D: pagable::PagableDeserializer<'de> + ?Sized>(
                &self,
                _deser: &mut D,
            ) -> pagable::Result<Self::Value> {
                Ok(None)
            }
        }
        T
    }
}

#[derive(Debug, Display, Hash, Eq, PartialEq, Clone, Allocative, Pagable)]
#[display("{}.{}", section, property)]
#[pagable_typetag(dice::DiceProjectionDyn)]
struct LegacyBsmrConfigPropertyProjectionKey {
    section: String,
    property: String,
}

impl ProjectionKey for LegacyBsmrConfigPropertyProjectionKey {
    type DeriveFromKey = LegacyBsmrConfigForCellKey;
    type Value = Option<Arc<str>>;

    fn compute(
        &self,
        config: &bsmr_error::Result<LegacyBsmrConfig>,
        _ctx: &DiceProjectionComputations,
    ) -> Option<Arc<str>> {
        // See the comment in `LegacyBsmrConfigErrorKey` for why this is safe
        let config = config.as_ref().unwrap();
        config
            .get(BsmrconfigKeyRef {
                section: &self.section,
                property: &self.property,
            })
            .map(|s| s.to_owned().into())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

impl HasInjectedLegacyConfigs for DiceComputations<'_> {
    async fn get_injected_external_bsmrconfig_data(
        &mut self,
    ) -> bsmr_error::Result<Arc<ExternalBsmrconfigData>> {
        self.compute(&LegacyExternalBsmrConfigDataKey).await?.ok_or_else(|| internal_error!(
            "Tried to retrieve LegacyExternalBsmrConfigDataKey from the graph, but key has None value"
        ))
    }

    async fn is_injected_external_bsmrconfig_data_key_set(&mut self) -> bsmr_error::Result<bool> {
        Ok(self
            .compute(&LegacyExternalBsmrConfigDataKey)
            .await?
            .is_some())
    }
}

pub fn inject_legacy_config_for_test(
    dice: &mut DiceTransactionUpdater,
    cell_name: CellName,
    configs: LegacyBsmrConfig,
) -> bsmr_error::Result<()> {
    dice.changed_to([(LegacyBsmrConfigForCellKey { cell_name }, Ok(configs))])?;
    dice.changed_to([(LegacyExternalBsmrConfigDataKey, None)])?;
    Ok(())
}

#[async_trait]
impl HasLegacyConfigs for DiceComputations<'_> {
    async fn get_legacy_config_on_dice(
        &mut self,
        cell_name: CellName,
    ) -> bsmr_error::Result<OpaqueLegacyBsmrConfigOnDice> {
        let config = self
            .compute_opaque(&LegacyBsmrConfigForCellKey { cell_name })
            .await?;
        if let Some(error) = self.projection(&config, &LegacyBsmrConfigErrorKey())? {
            return Err(error);
        }
        Ok(OpaqueLegacyBsmrConfigOnDice {
            config: Arc::new(config),
        })
    }

    async fn get_legacy_root_config_on_dice(
        &mut self,
    ) -> bsmr_error::Result<OpaqueLegacyBsmrConfigOnDice> {
        let cell_resolver = self.get_cell_resolver().await?;
        self.get_legacy_config_on_dice(cell_resolver.root_cell())
            .await
    }

    async fn get_legacy_config_for_cell(
        &mut self,
        cell_name: CellName,
    ) -> bsmr_error::Result<LegacyBsmrConfig> {
        self.compute(&LegacyBsmrConfigForCellKey { cell_name })
            .await?
    }

    async fn get_legacy_config_property(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<Arc<str>>> {
        self.get_legacy_config_on_dice(cell_name)
            .await?
            .lookup(self, key)
    }

    async fn parse_legacy_config_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static,
    {
        LegacyBsmrConfig::parse_value(
            key,
            self.get_legacy_config_property(cell_name, key)
                .await?
                .as_deref(),
        )
    }

    async fn parse_legacy_config_list_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: BsmrconfigKeyRef<'_>,
    ) -> bsmr_error::Result<Option<Vec<T>>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static,
    {
        LegacyBsmrConfig::parse_list_value(
            key,
            self.get_legacy_config_property(cell_name, key)
                .await?
                .as_deref(),
        )
    }
}

impl SetLegacyConfigs for DiceTransactionUpdater {
    fn set_legacy_config_external_data(
        &mut self,
        data: ExternalBsmrconfigData,
    ) -> bsmr_error::Result<()> {
        let data = data.filter_values(is_config_invisible_to_dice);
        Ok(self.changed_to(vec![(
            LegacyExternalBsmrConfigDataKey,
            Some(Arc::new(data)),
        )])?)
    }

    fn set_none_legacy_config_external_data(&mut self) -> bsmr_error::Result<()> {
        Ok(self.changed_to(vec![(LegacyExternalBsmrConfigDataKey, None)])?)
    }
}

fn is_config_invisible_to_dice(key: &BsmrconfigKeyRef) -> bool {
    !CONFIGS_INVISIBLE_TO_DICE.contains(key)
}

/// A set of bsmrconfigs that are visibile outside of dice, but not within it. Importantly, changes
/// to these configs do not cause state invalidations.
// FIXME(JakobDegen): Error if someone tries to read any of these from in dice
const CONFIGS_INVISIBLE_TO_DICE: &[BsmrconfigKeyRef<'static>] = &[
    BsmrconfigKeyRef {
        section: "bsmr_re_client",
        property: "override_use_case",
    },
    BsmrconfigKeyRef {
        section: "scuba",
        property: "defaults",
    },
];

#[cfg(test)]
mod tests {
    use bsmr_cli_proto::ConfigOverride;

    use crate::legacy_configs::configs::testing::parse_with_config_args;

    #[test]
    fn config_equals() -> bsmr_error::Result<()> {
        let path = "test";
        let config1 = parse_with_config_args(
            &[("test", "[sec1]\na=b\n[sec2]\nx=y")],
            path,
            &[ConfigOverride::flag_no_cell("sec1.a=c")],
        )?;

        let config2 = parse_with_config_args(&[("test", "[sec1]\na=c\n[sec2]\nx=y")], path, &[])?;

        let config3 = parse_with_config_args(
            &[("test", "[sec1]\na=b\n[sec2]\nx=y")],
            path,
            &[ConfigOverride::flag_no_cell("sec1.d=e")],
        )?;

        assert!(config1.compare(&config1));
        assert!(config2.compare(&config2));
        assert!(config3.compare(&config3));
        assert!(config1.compare(&config2));
        assert!(!config1.compare(&config3));
        assert!(!config2.compare(&config3));

        Ok(())
    }
}

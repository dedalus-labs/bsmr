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

use std::str::FromStr;
use std::sync::Arc;

use bsmr_error::BuckErrorContext;
use bsmr_hash::StdBuckHashMap;
use bsmr_util::env_vars::substitute_env_vars;
use gazebo::eq_chain;

use crate::legacy_configs::configs::ConfigValue;
use crate::legacy_configs::configs::LegacyBsmrConfig;
use crate::legacy_configs::configs::LegacyBsmrConfigSection;
use crate::legacy_configs::configs::LegacyBsmrConfigValue;
use crate::legacy_configs::key::BsmrconfigKeyRef;
use crate::legacy_configs::view::LegacyBsmrConfigView;

/// Read the `[bsmr_metadata]` section from a `LegacyBsmrConfig` and resolve any `$VAR`
/// references. Entries whose env vars are not set are skipped with a warning.
pub fn parse_bsmrconfig_metadata(config: &LegacyBsmrConfig) -> StdBuckHashMap<String, String> {
    let mut map = StdBuckHashMap::default();
    let Some(section) = config.get_section("bsmr_metadata") else {
        return map;
    };
    for (key, value) in section.iter() {
        match substitute_env_vars(value.as_str()) {
            Ok(resolved) => {
                map.insert(key.to_owned(), resolved);
            }
            Err(e) => {
                tracing::warn!("Skipping [bsmr_metadata] key `{}`: {:#}", key, e);
            }
        }
    }
    map
}

impl LegacyBsmrConfigView for &LegacyBsmrConfig {
    fn get(&mut self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<Arc<str>>> {
        Ok(LegacyBsmrConfig::get(self, key).map(|v| v.to_owned().into()))
    }
}

impl LegacyBsmrConfigSection {
    /// configs are equal if the data they resolve in is equal, regardless of the origin of the config
    pub(crate) fn compare(&self, other: &Self) -> bool {
        eq_chain!(
            self.values.len() == other.values.len(),
            self.values.iter().all(|(name, value)| other
                .values
                .get(name)
                .is_some_and(|other_val| other_val.as_str() == value.as_str()))
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, LegacyBsmrConfigValue<'_>)> {
        self.values
            .iter()
            .map(move |(key, value)| (key.as_str(), LegacyBsmrConfigValue { value }))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    pub fn get(&self, key: &str) -> Option<LegacyBsmrConfigValue<'_>> {
        self.values
            .get(key)
            .map(move |value| LegacyBsmrConfigValue { value })
    }
}

impl LegacyBsmrConfig {
    fn get_config_value(&self, key: BsmrconfigKeyRef) -> Option<&ConfigValue> {
        let BsmrconfigKeyRef { section, property } = key;
        self.0
            .values
            .get(section)
            .and_then(|s| s.values.get(property))
    }

    pub fn get(&self, key: BsmrconfigKeyRef) -> Option<&str> {
        self.get_config_value(key).map(|s| s.as_str())
    }

    /// Iterate all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, impl IntoIterator<Item = (&str, &str)>)> {
        self.0.values.iter().map(|(section, section_values)| {
            (
                section.as_str(),
                section_values
                    .values
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            )
        })
    }

    fn parse_impl<T: FromStr>(key: BsmrconfigKeyRef, value: &str) -> bsmr_error::Result<T>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        let BsmrconfigKeyRef { section, property } = key;
        value
            .parse()
            .map_err(bsmr_error::Error::from)
            .with_buck_error_context(|| {
                format!(
                    "Invalid value for bsmrconfig `{}.{}`: conversion to {} failed, value as `{}`",
                    section.to_owned(),
                    property.to_owned(),
                    std::any::type_name::<T>(),
                    value.to_owned(),
                )
            })
    }

    pub fn parse<T: FromStr>(&self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        self.get_config_value(key)
            .map(|s| {
                Self::parse_impl(key, s.as_str()).with_buck_error_context(|| {
                    format!("Defined {}", s.source.as_legacy_bsmr_config_location())
                })
            })
            .transpose()
    }

    pub fn parse_value<T: FromStr>(
        key: BsmrconfigKeyRef,
        value: Option<&str>,
    ) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        value.map(|s| Self::parse_impl(key, s)).transpose()
    }

    pub fn parse_list<T: FromStr>(
        &self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Vec<T>>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        Self::parse_list_value(key, self.get(key))
    }

    pub fn parse_list_value<T: FromStr>(
        key: BsmrconfigKeyRef,
        value: Option<&str>,
    ) -> bsmr_error::Result<Option<Vec<T>>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        /// A wrapper type so we can use .parse() on this.
        struct ParseList<T>(Vec<T>);

        impl<T> FromStr for ParseList<T>
        where
            T: FromStr,
        {
            type Err = <T as FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(
                    s.split(',').map(T::from_str).collect::<Result<_, _>>()?,
                ))
            }
        }

        Ok(Self::parse_value::<ParseList<T>>(key, value)?.map(|l| l.0))
    }

    pub fn sections(&self) -> impl Iterator<Item = &String> {
        self.0.values.keys()
    }

    pub fn all_sections(&self) -> impl Iterator<Item = (&String, &LegacyBsmrConfigSection)> + '_ {
        self.0.values.iter()
    }

    pub fn get_section(&self, section: &str) -> Option<&LegacyBsmrConfigSection> {
        self.0.values.get(section)
    }

    /// configs are equal if the data they resolve in is equal, regardless of the origin of the config
    pub(crate) fn compare(&self, other: &Self) -> bool {
        eq_chain!(
            self.0.values.len() == other.0.values.len(),
            self.0.values.iter().all(|(section_name, section)| {
                other
                    .0
                    .values
                    .get(section_name)
                    .is_some_and(|other_sec| other_sec.compare(section))
            })
        )
    }
}

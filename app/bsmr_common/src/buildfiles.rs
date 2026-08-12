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

use std::future::Future;
use std::sync::Arc;

use bsmr_core::cells::name::CellName;
use bsmr_fs::paths::file_name::FileNameBuf;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use pagable::Pagable;
use pagable::pagable_typetag;

use crate::legacy_configs::dice::HasLegacyConfigs;
use crate::legacy_configs::key::BsmrconfigKeyRef;
use crate::legacy_configs::view::LegacyBsmrConfigView;

const DEFAULT_BUILDFILE: &str = "BUILD.bsmr";

/// Resolve the cell's single package build-file name.
pub fn parse_buildfile_name(
    mut config: impl LegacyBsmrConfigView,
) -> bsmr_error::Result<Vec<FileNameBuf>> {
    if config
        .parse::<String>(BsmrconfigKeyRef {
            section: "buildfile",
            property: "name_v2",
        })?
        .is_some()
    {
        return Err(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "`buildfile.name_v2` is no longer supported; use `buildfile.name`"
        ));
    }

    let buildfile = config
        .parse::<String>(BsmrconfigKeyRef {
            section: "buildfile",
            property: "name",
        })?
        .unwrap_or_else(|| DEFAULT_BUILDFILE.to_owned());

    Ok(vec![FileNameBuf::try_from(buildfile)?])
}

pub trait HasBuildfiles {
    fn get_buildfiles(
        &mut self,
        cell: CellName,
    ) -> impl Future<Output = bsmr_error::Result<Arc<[FileNameBuf]>>>;
}

#[derive(
    Clone,
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    PartialEq,
    allocative::Allocative,
    Pagable
)]
#[display("BuildfilesKey({})", self.0)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct BuildfilesKey(CellName);

#[async_trait::async_trait]
impl Key for BuildfilesKey {
    type Value = bsmr_error::Result<Arc<[FileNameBuf]>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let config = ctx.get_legacy_config_on_dice(self.0).await?;
        Ok(parse_buildfile_name(config.view(ctx))?.into())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

impl HasBuildfiles for DiceComputations<'_> {
    async fn get_buildfiles(&mut self, cell: CellName) -> bsmr_error::Result<Arc<[FileNameBuf]>> {
        self.compute(&BuildfilesKey(cell)).await?
    }
}

#[cfg(test)]
mod tests {
    use bsmr_core::cells::name::CellName;
    use gazebo::prelude::SliceExt;
    use indoc::indoc;

    use crate::buildfiles::parse_buildfile_name;
    use crate::legacy_configs::cells::BsmrConfigBasedCells;
    use crate::legacy_configs::configs::testing::TestConfigParserFileOps;

    #[tokio::test]
    async fn test_buildfiles() -> bsmr_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".bsmr",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                                third_party = third_party/
                        "#
                ),
            ),
            (
                "other/.bsmr",
                indoc!(
                    r#"
                            [cells]
                                other = .
                            [buildfile]
                                name = TARGETS
                        "#
                ),
            ),
            (
                "third_party/.bsmr",
                indoc!(
                    r#"
                            [cells]
                                third_party = .
                            [buildfile]
                                name_v2 = OKAY
                        "#
                ),
            ),
        ])?;

        let cells = BsmrConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["BUILD.bsmr"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["TARGETS"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("third_party"), &mut file_ops)
            .await?;
        let error = parse_buildfile_name(&config).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("`buildfile.name_v2` is no longer supported")
        );

        Ok(())
    }
}

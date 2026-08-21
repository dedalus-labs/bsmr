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

use std::io::Write;

use bsmr_hash::BsmrIndexMap;
use regex::RegexSet;

use super::output_path_parser::OutputPathType;

pub(crate) struct OutputPathTypePrinter {
    json: bool,
    attributes: Option<RegexSet>,
}

impl OutputPathTypePrinter {
    pub(crate) fn new(json: bool, attributes: &Vec<String>) -> bsmr_error::Result<Self> {
        let attributes = if attributes.is_empty() {
            None
        } else {
            Some(RegexSet::new(attributes)?)
        };

        Ok(OutputPathTypePrinter { json, attributes })
    }

    pub(crate) fn print(
        &self,
        path_type: &OutputPathType,
        mut stdout: impl Write,
    ) -> bsmr_error::Result<()> {
        if self.json {
            writeln!(
                &mut stdout,
                "{}",
                serde_json::to_string_pretty(&self.printable_attributes(path_type))?
            )?;
        } else {
            self.printable_attributes(path_type)
                .values()
                .try_for_each(|a| writeln!(&mut stdout, "{a}"))?;
        }
        Ok(())
    }

    fn printable_attributes(&self, path_type: &OutputPathType) -> BsmrIndexMap<String, String> {
        let all_attributes = self.all_attributes(path_type);

        if let Some(attributes) = &self.attributes {
            all_attributes
                .into_iter()
                .filter(|(k, _)| attributes.is_match(k))
                .collect()
        } else {
            all_attributes
        }
    }

    fn all_attributes(&self, path_type: &OutputPathType) -> BsmrIndexMap<String, String> {
        // Deterministic order
        let mut attributes = BsmrIndexMap::default();

        match path_type {
            OutputPathType::BxlOutput {
                bxl_function_label,
                common_attrs,
            } => {
                attributes.insert(
                    "bxl_function_label".to_owned(),
                    bxl_function_label.to_string(),
                );
                if let Some(config_hash) = &common_attrs.config_hash {
                    attributes.insert("config_hash".to_owned(), config_hash.clone());
                }
                if let Some(content_hash) = &common_attrs.content_hash {
                    attributes.insert("content_hash".to_owned(), content_hash.clone());
                }
                attributes.insert(
                    "full_artifact_path_no_hash".to_owned(),
                    common_attrs.raw_path_to_output.to_string(),
                );
            }
            OutputPathType::AnonOutput {
                path,
                target_label,
                attr_hash,
                common_attrs,
            } => {
                attributes.insert("cell_path".to_owned(), path.to_string());
                attributes.insert("target_label".to_owned(), target_label.to_string());
                attributes.insert("attr_hash".to_owned(), attr_hash.clone());
                if let Some(config_hash) = &common_attrs.config_hash {
                    attributes.insert("config_hash".to_owned(), config_hash.clone());
                }
                if let Some(content_hash) = &common_attrs.content_hash {
                    attributes.insert("content_hash".to_owned(), content_hash.clone());
                }
                attributes.insert(
                    "full_artifact_path_no_hash".to_owned(),
                    common_attrs.raw_path_to_output.to_string(),
                );
            }
            OutputPathType::RuleOutput {
                path,
                target_label,
                short_path,
                common_attrs,
            } => {
                attributes.insert("cell_path".to_owned(), path.to_string());
                attributes.insert("target_label".to_owned(), target_label.to_string());
                attributes.insert("short_artifact_path".to_owned(), short_path.to_string());
                if let Some(config_hash) = &common_attrs.config_hash {
                    attributes.insert("config_hash".to_owned(), config_hash.clone());
                }
                if let Some(content_hash) = &common_attrs.content_hash {
                    attributes.insert("content_hash".to_owned(), content_hash.clone());
                }
                attributes.insert(
                    "full_artifact_path_no_hash".to_owned(),
                    common_attrs.raw_path_to_output.to_string(),
                );
            }
            OutputPathType::TestOutput { path, common_attrs } => {
                attributes.insert("cell_path".to_owned(), path.to_string());
                if let Some(config_hash) = &common_attrs.config_hash {
                    attributes.insert("config_hash".to_owned(), config_hash.clone());
                }
                if let Some(content_hash) = &common_attrs.content_hash {
                    attributes.insert("content_hash".to_owned(), content_hash.clone());
                }
                attributes.insert(
                    "full_artifact_path_no_hash".to_owned(),
                    common_attrs.raw_path_to_output.to_string(),
                );
            }
            OutputPathType::TmpOutput {
                path,
                target_label,
                common_attrs,
            } => {
                attributes.insert("cell_path".to_owned(), path.to_string());
                attributes.insert("target_label".to_owned(), target_label.to_string());
                if let Some(config_hash) = &common_attrs.config_hash {
                    attributes.insert("config_hash".to_owned(), config_hash.clone());
                }
                if let Some(content_hash) = &common_attrs.content_hash {
                    attributes.insert("content_hash".to_owned(), content_hash.clone());
                }
                attributes.insert(
                    "full_artifact_path_no_hash".to_owned(),
                    common_attrs.raw_path_to_output.to_string(),
                );
            }
        }

        attributes
    }
}

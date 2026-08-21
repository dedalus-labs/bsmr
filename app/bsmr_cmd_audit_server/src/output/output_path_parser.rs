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

use std::iter::Peekable;

use bsmr_build_api::bxl::types::BxlFunctionLabel;
use bsmr_core::bxl::BxlFilePath;
use bsmr_core::cells::CellResolver;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePath;
use bsmr_core::fs::output_path::BSMR_OUTPUT_ROOT;
use bsmr_core::package::PackageLabel;
use bsmr_core::target::label::label::TargetLabel;
use bsmr_core::target::name::EQ_SIGN_SUBST;
use bsmr_core::target::name::TargetNameRef;
use bsmr_error::BsmrErrorContext;
use bsmr_error::bsmr_error;
use bsmr_error::internal_error;
use bsmr_fs::paths::file_name::FileName;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use dupe::Dupe;
use itertools::Itertools;

/// The common attributes of each `bsmr-out` path type,
pub(crate) struct OutputPathTypeCommon {
    /// Configuration hash within the `bsmr-out` path, if present.
    pub(crate) config_hash: Option<String>,
    /// Content hash within the `bsmr-out` path, if present.
    pub(crate) content_hash: Option<String>,
    /// The path starting from cell to the artifact, without the configuration hash. For example, in
    /// `bsmr-out/default/art/cell/<CONFIG_HASH>/path/to/__target_name__/target`, it would be `cell/path/to/__target_name__/target`.
    pub(crate) raw_path_to_output: ForwardRelativePathBuf,
}

/// The types of the `bsmr-out` path.
pub(crate) enum OutputPathType {
    BxlOutput {
        // `BxlFunctionLabel` contains the `CellPath` to the bxl function.
        bxl_function_label: BxlFunctionLabel,
        common_attrs: OutputPathTypeCommon,
    },
    AnonOutput {
        path: CellPath,
        target_label: TargetLabel,
        // Rule attr hash is part of anonymous target bsmr-outs.
        attr_hash: String,
        common_attrs: OutputPathTypeCommon,
    },
    RuleOutput {
        path: CellPath,
        target_label: TargetLabel,
        // This is the part of the bsmr-out after target name. For example, it would `artifact` in  `gen/path/to/__target_name__/artifact`
        short_path: ForwardRelativePathBuf,
        common_attrs: OutputPathTypeCommon,
    },
    TestOutput {
        path: CellPath,
        common_attrs: OutputPathTypeCommon,
    },
    TmpOutput {
        path: CellPath,
        target_label: TargetLabel,
        common_attrs: OutputPathTypeCommon,
    },
}

pub(crate) struct OutputPathParser {
    cell_resolver: CellResolver,
}

fn validate_output_root<'v>(
    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,
) -> bsmr_error::Result<()> {
    // Validate that this path belongs to Bessemer before interpreting its layout.
    match iter.next() {
        Some(output_root) if output_root == BSMR_OUTPUT_ROOT => {}
        Some(_) | None => {
            return Err(bsmr_error!(
                bsmr_error::ErrorTag::Input,
                "Path does not start with `{BSMR_OUTPUT_ROOT}`"
            ));
        }
    }

    // Advance the iterator to isolation dir.
    match iter.next() {
        Some(_) => Ok(()),
        None => Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Path does not have an isolation dir"
        )),
    }
}

struct OutputPathData {
    // Cell path of the target label that created the artifact.
    cell_path: CellPath,
    config_hash: Option<String>,
    content_hash: Option<String>,
    anon_hash: Option<String>,
    /// The path starting from cell to the artifact, without the configuration hash. For example, in
    /// `bsmr-out/default/art/cell/<CONFIG_HASH>/path/to/__target_name__/target`, it would be `cell/path/to/__target_name__/target`.
    raw_path_to_output: ForwardRelativePathBuf,
}

fn is_hash(s: &str) -> bool {
    if s.len() != 16 {
        return false;
    }

    for c in s.chars() {
        if !c.is_ascii_hexdigit() {
            return false;
        }
    }

    true
}

fn get_cell_path<'v>(
    iter: &mut Peekable<impl Iterator<Item = &'v FileName> + Clone>,
    cell_resolver: &'v CellResolver,
    generated_prefix: &'v str,
) -> bsmr_error::Result<OutputPathData> {
    let is_anon = generated_prefix == "art-anon" || generated_prefix == "gen-anon";
    let is_test = generated_prefix == "test";
    // Get cell name and validate it exists
    let Some(cell_name) = iter.next() else {
        return Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Invalid cell name"
        ));
    };

    let cell_name = CellName::unchecked_new(cell_name.as_str())?;
    let mut raw_path_to_output = ForwardRelativePath::new(cell_name.as_str())?.to_buf();

    cell_resolver.get(cell_name)?;

    let Some(potential_config_hash) = iter.peek() else {
        return Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Path does not have a platform configuration or content-based hash"
        ));
    };

    let potential_config_hash_string = potential_config_hash.to_string();
    let config_hash = if is_hash(potential_config_hash_string.as_str()) {
        // Advance the iterator if it is a config hash
        iter.next();
        Some(potential_config_hash_string)
    } else {
        None
    };

    // If we found a config hash, then the raw_path_to_output is just the remaining path.
    // If we didn't find a config hash, then there is a content hash in the remaining path.
    // We need to (a) extract the content hash, and (b) construct the raw_path_to_output
    // from all of the path segments except for the content hash.
    let mut content_hash = None;
    let mut found_hash = config_hash.is_some();
    iter.clone().for_each(|f| {
        if found_hash {
            raw_path_to_output.push(f);
        } else {
            let is_content_hash = is_hash(f.as_str());
            if is_content_hash {
                content_hash = Some(f.to_string());
                found_hash = true;
            } else {
                raw_path_to_output.push(f);
            }
        }
    });

    if !found_hash {
        return Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Path does not have a platform configuration or content-based hash"
        ));
    };

    // Get cell relative path and construct the cell path
    let mut cell_relative_path = CellRelativePath::unchecked_new("").to_owned();

    while let Some(maybe_target_name) = iter.peek() {
        if !maybe_target_name.as_str().starts_with("__") {
            cell_relative_path.push(maybe_target_name);
            iter.next();
            continue;
        }
        // Intentionally leave the target label on the iterator

        // If it's an anonymous target, then the last part before the target name is actually the
        // hash, and not part of the cell relative path.
        let (cell_relative_path, anon_hash) = if is_anon {
            let path = cell_relative_path
                .parent()
                .ok_or_else(|| internal_error!("Invalid path for anonymous target"))?
                .to_buf();
            let anon_hash = cell_relative_path.file_name().unwrap().as_str().to_owned();
            (path, Some(anon_hash))
        } else {
            (cell_relative_path.to_buf(), None)
        };
        let cell_path = CellPath::new(cell_name, cell_relative_path);

        let output_path_data = OutputPathData {
            cell_path,
            config_hash,
            content_hash,
            anon_hash,
            raw_path_to_output: raw_path_to_output.to_buf(),
        };

        return Ok(output_path_data);
    }

    if is_test {
        let output_path_data = OutputPathData {
            cell_path: CellPath::new(cell_name, cell_relative_path.to_buf()),
            config_hash,
            content_hash,
            anon_hash: None,
            raw_path_to_output: raw_path_to_output.to_buf(),
        };
        Ok(output_path_data)
    } else {
        Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Invalid target name"
        ))
    }
}

fn get_target_name<'v>(
    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,
) -> bsmr_error::Result<String> {
    // Get target name, which is prefixed and suffixed with "__"
    match iter.next() {
        Some(raw_target_name) => {
            let mut target_name_with_underscores =
                <&ForwardRelativePath>::from(raw_target_name).to_owned();

            while !target_name_with_underscores.as_str().ends_with("__") {
                match iter.next() {
                    Some(next) => {
                        target_name_with_underscores = target_name_with_underscores.join(next);
                    }
                    None => {
                        return Err(bsmr_error!(
                            bsmr_error::ErrorTag::Input,
                            "Invalid target name"
                        ));
                    }
                }
            }

            let target_name_with_underscores = target_name_with_underscores.as_str();
            let target_name =
                &target_name_with_underscores[2..(target_name_with_underscores.len() - 2)];
            Ok(target_name.replace(EQ_SIGN_SUBST, "="))
        }
        None => Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Invalid target name"
        )),
    }
}

fn get_target_label<'v>(
    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,
    path: CellPath,
) -> bsmr_error::Result<TargetLabel> {
    let target_name = get_target_name(iter)?;
    let package = PackageLabel::from_cell_path(path.as_ref())?;
    let target = TargetNameRef::new(target_name.as_str())?;
    let target_label = TargetLabel::new(package.dupe(), target);
    Ok(target_label)
}

fn get_bxl_function_label<'v>(
    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,
    path: CellPath,
) -> bsmr_error::Result<BxlFunctionLabel> {
    let target_name = get_target_name(iter)?;
    let bxl_path = BxlFilePath::new(path)?;
    let bxl_function_label = BxlFunctionLabel {
        bxl_path,
        name: target_name,
    };

    Ok(bxl_function_label)
}

impl OutputPathParser {
    pub(crate) fn new(cell_resolver: CellResolver) -> OutputPathParser {
        OutputPathParser { cell_resolver }
    }

    // Validates and parses the bsmr-out path, returning the `OutputPathType`. Assumes
    // that the inputted path is not a symlink.
    pub(crate) fn parse(&self, output_path: &str) -> bsmr_error::Result<OutputPathType> {
        let path_as_forward_rel_path = ForwardRelativePathBuf::new(output_path.to_owned())?;
        let mut iter = path_as_forward_rel_path.iter().peekable();

        validate_output_root(&mut iter)?;

        self.parse_after_isolation_dir(iter).with_bsmr_error_context(||
            format!(
                "Malformed output path. Expected format: `{BSMR_OUTPUT_ROOT}/<isolation_prefix>/<gen|tmp|test|gen-anon|gen-bxl>/<cell_name>/<cfg_hash>/<target_path?>/__<target_name>__/<__action__id__?>/<outputs>`. Actual path was: `{}`",
                output_path,
            )
        )
    }

    fn parse_after_isolation_dir<'v>(
        &'v self,
        mut iter: Peekable<impl Iterator<Item = &'v FileName> + Clone>,
    ) -> bsmr_error::Result<OutputPathType> {
        // Advance the iterator to the prefix (tmp, test, gen, art-anon, or art-bxl)
        match iter.next() {
            Some(part) => {
                let result = match part.as_str() {
                    "tmp" => {
                        let output_path_data =
                            get_cell_path(&mut iter, &self.cell_resolver, "tmp")?;
                        let target_label =
                            get_target_label(&mut iter, output_path_data.cell_path.clone())?;

                        let common_attrs = OutputPathTypeCommon {
                            config_hash: output_path_data.config_hash,
                            content_hash: output_path_data.content_hash,
                            raw_path_to_output: output_path_data.raw_path_to_output,
                        };

                        Ok(OutputPathType::TmpOutput {
                            path: output_path_data.cell_path,
                            target_label,
                            common_attrs,
                        })
                    }
                    "test" => {
                        let output_path_data =
                            get_cell_path(&mut iter, &self.cell_resolver, "test")?;

                        let common_attrs = OutputPathTypeCommon {
                            config_hash: output_path_data.config_hash,
                            content_hash: output_path_data.content_hash,
                            raw_path_to_output: output_path_data.raw_path_to_output,
                        };

                        Ok(OutputPathType::TestOutput {
                            path: output_path_data.cell_path,
                            common_attrs,
                        })
                    }
                    "gen" | "art" => {
                        let output_path_data =
                            get_cell_path(&mut iter, &self.cell_resolver, part.as_str())?;
                        let target_label =
                            get_target_label(&mut iter, output_path_data.cell_path.clone())?;
                        if let Some(potential_config_hash) = iter.peek() {
                            if is_hash(potential_config_hash.as_str()) {
                                iter.next();
                            }
                        }
                        let path_after_target_name =
                            ForwardRelativePathBuf::new(iter.clone().join("/"))?;
                        let common_attrs = OutputPathTypeCommon {
                            config_hash: output_path_data.config_hash,
                            content_hash: output_path_data.content_hash,
                            raw_path_to_output: output_path_data.raw_path_to_output,
                        };

                        Ok(OutputPathType::RuleOutput {
                            path: output_path_data.cell_path,
                            target_label,
                            short_path: path_after_target_name,
                            common_attrs,
                        })
                    }
                    "art-anon" => {
                        let output_path_data =
                            get_cell_path(&mut iter, &self.cell_resolver, part.as_str())?;
                        let target_label =
                            get_target_label(&mut iter, output_path_data.cell_path.clone())?;
                        let common_attrs = OutputPathTypeCommon {
                            config_hash: output_path_data.config_hash,
                            content_hash: output_path_data.content_hash,
                            raw_path_to_output: output_path_data.raw_path_to_output,
                        };

                        Ok(OutputPathType::AnonOutput {
                            path: output_path_data.cell_path,
                            target_label,
                            attr_hash: output_path_data
                                .anon_hash
                                .expect("No hash found in anonymous artifact bsmr-out"),
                            common_attrs,
                        })
                    }
                    "art-bxl" => {
                        let output_path_data =
                            get_cell_path(&mut iter, &self.cell_resolver, part.as_str())?;
                        let bxl_function_label =
                            get_bxl_function_label(&mut iter, output_path_data.cell_path)?;
                        let common_attrs = OutputPathTypeCommon {
                            config_hash: output_path_data.config_hash,
                            content_hash: output_path_data.content_hash,
                            raw_path_to_output: output_path_data.raw_path_to_output,
                        };

                        Ok(OutputPathType::BxlOutput {
                            bxl_function_label,
                            common_attrs,
                        })
                    }
                    _ => Err(bsmr_error!(
                        bsmr_error::ErrorTag::InvalidOutputPath,
                        "Directory after isolation dir is invalid (should be gen, art, art-bxl, art-anon, tmp, or test)"
                    )),
                };

                // Validate for non-test outputs that the target name is not the last element in the path
                if part != "test" && iter.peek().is_none() {
                    Err(bsmr_error!(
                        bsmr_error::ErrorTag::InvalidOutputPath,
                        "No output artifacts found"
                    ))
                } else {
                    result
                }
            }
            None => Err(bsmr_error!(
                bsmr_error::ErrorTag::InvalidOutputPath,
                "Path is empty"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bsmr_build_api::bxl::types::BxlFunctionLabel;
    use bsmr_core::bxl::BxlFilePath;
    use bsmr_core::cells::CellResolver;
    use bsmr_core::cells::cell_path::CellPath;
    use bsmr_core::cells::cell_root_path::CellRootPath;
    use bsmr_core::cells::name::CellName;
    use bsmr_core::cells::paths::CellRelativePath;
    use bsmr_core::configuration::data::ConfigurationData;
    use bsmr_core::configuration::data::ConfigurationDataData;
    use bsmr_core::fs::project_rel_path::ProjectRelativePath;
    use bsmr_core::package::PackageLabel;
    use bsmr_core::target::label::label::TargetLabel;
    use bsmr_core::target::name::TargetNameRef;
    use bsmr_fs::paths::forward_rel_path::ForwardRelativePathBuf;
    use dupe::Dupe;

    use crate::output::output_path_parser::OutputPathParser;
    use crate::output::output_path_parser::OutputPathType;

    fn get_test_data() -> (OutputPathParser, String, TargetLabel, CellPath) {
        let cell_path = CellRootPath::new(ProjectRelativePath::new("foo/bar").unwrap());
        let cell_resolver = CellResolver::testing_with_name_and_path(
            CellName::testing_new("bar"),
            cell_path.to_buf(),
        );
        let parser = OutputPathParser::new(cell_resolver);

        let configuration = ConfigurationData::from_platform(
            "cfg_for//:testing_exec".to_owned(),
            ConfigurationDataData {
                constraints: BTreeMap::new(),
            },
            false,
        )
        .unwrap();
        let config_hash = configuration.output_hash().to_string();

        let pkg = PackageLabel::new(
            CellName::testing_new("bar"),
            CellRelativePath::unchecked_new("path/to/target"),
        )
        .unwrap();
        let expected_target_label =
            TargetLabel::new(pkg, TargetNameRef::new("target_name").unwrap());
        let expected_cell_path = CellPath::new(
            CellName::testing_new("bar"),
            CellRelativePath::unchecked_new("path/to/target").to_owned(),
        );

        (
            parser,
            config_hash,
            expected_target_label,
            expected_cell_path,
        )
    }

    #[test]
    fn test_validation() -> bsmr_error::Result<()> {
        let (output_parser, config_hash, _, _) = get_test_data();

        let malformed_path1 = "does/not/start/with/bsmr-out/blah/blah";
        let malformed_path2 = "bsmr-out/default/invalid_bsmr_prefix/blah/blah/blah/blah";
        let malformed_path3 = "bsmr-out/default/art/bar/no/target/name/found";
        let malformed_path4 = "bsmr-out/default/art/bar/path/to/target/__but_no_artifacts__";

        let res = output_parser.parse(malformed_path1);
        assert!(
            res.err()
                .unwrap()
                .to_string()
                .contains("Path does not start with")
        );

        let res = output_parser.parse(malformed_path2);
        assert!(res.err().unwrap().to_string().contains("Malformed"));

        let res = output_parser.parse(malformed_path3);
        assert!(res.err().unwrap().to_string().contains("Malformed"));

        let res = output_parser.parse(malformed_path4);
        assert!(res.err().unwrap().to_string().contains("Malformed"));

        let cell_does_not_exist =
            "bsmr-out/default/art/nonexistent_cell/cfg_hash/path/to/target/__target_name__/output";

        let res = output_parser.parse(cell_does_not_exist);
        assert!(res.err().unwrap().to_string().contains("Malformed"));

        let no_artifacts_after_target_name =
            &format!("bsmr-out/default/art/bar/{config_hash}/path/to/target/__target_name__");
        let res = output_parser.parse(no_artifacts_after_target_name);
        assert!(res.err().unwrap().to_string().contains("Malformed"));

        Ok(())
    }

    #[test]
    fn test_target_output() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let rule_path = format!(
            "bsmr-out/default/art/bar/{expected_config_hash}/path/to/target/__target_name__/output"
        );

        let res = output_parser.parse(&rule_path)?;

        match res {
            OutputPathType::RuleOutput {
                path,
                target_label,
                short_path,
                common_attrs,
            } => {
                assert_eq!(
                    short_path,
                    ForwardRelativePathBuf::new("output".to_owned())?,
                );
                assert_eq!(target_label, expected_target_label);
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_target_content_based_output() -> bsmr_error::Result<()> {
        let (output_parser, _expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let content_based_hash = "0123456789abcdef";
        let rule_path = format!(
            "bsmr-out/default/art/bar/path/to/target/__target_name__/{content_based_hash}/output"
        );

        let res = output_parser.parse(&rule_path)?;

        match res {
            OutputPathType::RuleOutput {
                path,
                target_label,
                short_path,
                common_attrs,
            } => {
                assert_eq!(
                    short_path,
                    ForwardRelativePathBuf::new("output".to_owned())?,
                );
                assert_eq!(target_label, expected_target_label);
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, None);
                assert_eq!(
                    common_attrs.content_hash,
                    Some(content_based_hash.to_owned())
                );
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_target_output_with_slashes() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let rule_path_target_label_with_slashes = format!(
            "bsmr-out/default/art/bar/{expected_config_hash}/path/to/target/__target_name_start/target_name_end__/output"
        );

        let res = output_parser.parse(&rule_path_target_label_with_slashes)?;

        let expected_target_label_with_slashes = TargetLabel::new(
            expected_target_label.pkg().dupe(),
            TargetNameRef::new("target_name_start/target_name_end")?,
        );

        match res {
            OutputPathType::RuleOutput {
                path,
                target_label,
                short_path,
                common_attrs,
            } => {
                assert_eq!(
                    short_path,
                    ForwardRelativePathBuf::new("output".to_owned())?,
                );
                assert_eq!(target_label, expected_target_label_with_slashes);
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name_start/target_name_end__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_target_output_with_eq_sign() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let rule_path_with_equal_sign = format!(
            "bsmr-out/default/art/bar/{expected_config_hash}/path/to/target/__target_name_eqsb_out__/output"
        );

        let res = output_parser.parse(&rule_path_with_equal_sign)?;

        let expected_target_label_with_equal_sign = TargetLabel::new(
            expected_target_label.pkg(),
            TargetNameRef::new("target_name=out")?,
        );

        match res {
            OutputPathType::RuleOutput {
                path,
                target_label,
                short_path,
                common_attrs,
            } => {
                assert_eq!(
                    short_path,
                    ForwardRelativePathBuf::new("output".to_owned())?,
                );
                assert_eq!(target_label, expected_target_label_with_equal_sign);
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name_eqsb_out__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_tmp_output() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let tmp_path = format!(
            "bsmr-out/default/tmp/bar/{expected_config_hash}/path/to/target/__target_name__/output"
        );

        let res = output_parser.parse(&tmp_path)?;

        match res {
            OutputPathType::TmpOutput {
                path,
                target_label,
                common_attrs,
            } => {
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(target_label, expected_target_label);
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_tmp_content_based_output() -> bsmr_error::Result<()> {
        let (output_parser, _expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let content_based_hash = "0123456789abcdef";

        let tmp_path = format!(
            "bsmr-out/default/tmp/bar/path/to/target/__target_name__/{content_based_hash}/output"
        );

        let res = output_parser.parse(&tmp_path)?;

        match res {
            OutputPathType::TmpOutput {
                path,
                target_label,
                common_attrs,
            } => {
                assert_eq!(path, expected_cell_path);
                assert_eq!(common_attrs.config_hash, None);
                assert_eq!(
                    common_attrs.content_hash,
                    Some(content_based_hash.to_owned())
                );
                assert_eq!(target_label, expected_target_label);
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_test_output() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, _, _) = get_test_data();

        let test_path =
            format!("bsmr-out/default/test/bar/{expected_config_hash}/path/to/target/test/output");

        let expected_test_cell_path = CellPath::new(
            CellName::testing_new("bar"),
            CellRelativePath::unchecked_new("path/to/target/test/output").to_owned(),
        );

        let res = output_parser.parse(&test_path)?;

        match res {
            OutputPathType::TestOutput { path, common_attrs } => {
                assert_eq!(path, expected_test_cell_path);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/test/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_anon_output() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let anon_path = format!(
            "bsmr-out/default/art-anon/bar/{expected_config_hash}/path/to/target/anon_hash/__target_name__/output"
        );

        let res = output_parser.parse(&anon_path)?;

        match res {
            OutputPathType::AnonOutput {
                path,
                target_label,
                attr_hash,
                common_attrs,
            } => {
                assert_eq!(target_label, expected_target_label);
                assert_eq!(path, expected_cell_path);
                assert_eq!(attr_hash, "anon_hash");
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/anon_hash/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_anon_content_based_output() -> bsmr_error::Result<()> {
        let (output_parser, _expected_config_hash, expected_target_label, expected_cell_path) =
            get_test_data();

        let content_based_hash = "0123456789abcdef";

        let anon_path = format!(
            "bsmr-out/default/art-anon/bar/path/to/target/anon_hash/__target_name__/{content_based_hash}/output"
        );

        let res = output_parser.parse(&anon_path)?;

        match res {
            OutputPathType::AnonOutput {
                path,
                target_label,
                attr_hash,
                common_attrs,
            } => {
                assert_eq!(target_label, expected_target_label);
                assert_eq!(path, expected_cell_path);
                assert_eq!(attr_hash, "anon_hash");
                assert_eq!(common_attrs.config_hash, None);
                assert_eq!(
                    common_attrs.content_hash,
                    Some(content_based_hash.to_owned())
                );
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/target/anon_hash/__target_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_bxl_output() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, _, _) = get_test_data();

        let path = format!(
            "bsmr-out/default/art-bxl/bar/{expected_config_hash}/path/to/function.bxl/__function_name__/output"
        );

        let res = output_parser.parse(&path)?;

        match res {
            OutputPathType::BxlOutput {
                bxl_function_label,
                common_attrs,
            } => {
                let path = CellPath::new(
                    CellName::testing_new("bar"),
                    CellRelativePath::unchecked_new("path/to/function.bxl").to_owned(),
                );

                let bxl_path = BxlFilePath::new(path)?;
                let expected_bxl_function_label = BxlFunctionLabel {
                    bxl_path,
                    name: "function_name".to_owned(),
                };

                assert_eq!(bxl_function_label, expected_bxl_function_label);
                assert_eq!(common_attrs.config_hash, Some(expected_config_hash));
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/function.bxl/__function_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_bxl_content_based_output() -> bsmr_error::Result<()> {
        let (output_parser, _, _, _) = get_test_data();
        let content_based_hash = "0123456789abcdef";

        let path = format!(
            "bsmr-out/default/art-bxl/bar/path/to/function.bxl/__function_name__/{content_based_hash}/output"
        );

        let res = output_parser.parse(&path)?;

        match res {
            OutputPathType::BxlOutput {
                bxl_function_label,
                common_attrs,
            } => {
                let path = CellPath::new(
                    CellName::testing_new("bar"),
                    CellRelativePath::unchecked_new("path/to/function.bxl").to_owned(),
                );

                let bxl_path = BxlFilePath::new(path)?;
                let expected_bxl_function_label = BxlFunctionLabel {
                    bxl_path,
                    name: "function_name".to_owned(),
                };

                assert_eq!(bxl_function_label, expected_bxl_function_label);
                assert_eq!(common_attrs.config_hash, None);
                assert_eq!(
                    common_attrs.content_hash,
                    Some(content_based_hash.to_owned())
                );
                assert_eq!(
                    common_attrs.raw_path_to_output.as_str(),
                    "bar/path/to/function.bxl/__function_name__/output"
                )
            }
            _ => panic!("Should have parsed bsmr-out path successfully"),
        }

        Ok(())
    }

    #[test]
    fn test_empty_package_path() -> bsmr_error::Result<()> {
        let (output_parser, expected_config_hash, _, _) = get_test_data();

        let target_path =
            format!("bsmr-out/default/art/bar/{expected_config_hash}/__target_name__/output");

        let OutputPathType::RuleOutput {
            path, target_label, ..
        } = output_parser.parse(&target_path)?
        else {
            panic!("Should have parsed bsmr-out path successfully")
        };

        assert!(path.path().is_empty());
        assert_eq!(target_label.name().as_str(), "target_name");

        Ok(())
    }
}

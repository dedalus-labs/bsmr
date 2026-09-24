//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Identify private Cargo graphs as direct children of their real source package.

use allocative::Allocative;
use bsmr_core::package::PackageLabel;
use bsmr_fs::paths::file_name::FileName;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use pagable::Pagable;

const PREFIX: &str = "__bsmr_cargo_";

/// The Cargo operation intrinsic to one public entrypoint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
pub enum Mode {
    Build,
    Test,
}

impl Mode {
    /// Return the stable descriptor and planner spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
        }
    }
}

/// A named Cargo target, validated against its package's tracked target catalog.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
pub enum Target {
    Lib(String),
    Bin(String),
    Test(String),
}

impl Target {
    /// Return the Cargo target kind used by descriptors and catalog validation.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Lib(_) => "lib",
            Self::Bin(_) => "bin",
            Self::Test(_) => "test",
        }
    }

    /// Return the original Cargo target name without normalizing its punctuation.
    pub fn name(&self) -> &str {
        match self {
            Self::Lib(name) | Self::Bin(name) | Self::Test(name) => name,
        }
    }

    /// Format the reserved direct-child directory for this target and operation.
    pub fn child_name(&self, mode: Mode) -> String {
        format!("{PREFIX}{}_{}_{}", mode.as_str(), self.kind(), self.name())
    }
}

/// A private graph request whose package is the physical parent, not the virtual path.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
pub struct Entry {
    /// The physical source package whose tracked catalog owns this entry.
    pub package: PackageLabel,
    /// Whether the entry compiles a build product or a test harness.
    pub mode: Mode,
    /// The original Cargo target identity, preserving punctuation.
    pub target: Target,
}

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum EntryError {
    #[error("invalid private Cargo package descriptor `{0}`")]
    Invalid(PackageLabel),
}

impl Entry {
    /// Decode a reserved direct-child package, rejecting malformed reserved names.
    pub fn parse(package: PackageLabel) -> bsmr_error::Result<Option<Self>> {
        let Some(leaf) = package.as_cell_path().path().file_name() else {
            return Ok(None);
        };
        let Some(suffix) = leaf.as_str().strip_prefix(PREFIX) else {
            return Ok(None);
        };
        let invalid = || EntryError::Invalid(package);
        let mut parts = suffix.splitn(3, '_');
        let mode = match parts.next() {
            Some("build") => Mode::Build,
            Some("test") => Mode::Test,
            _ => return Err(invalid().into()),
        };
        let kind = parts.next();
        let name = parts
            .next()
            .filter(|name| !name.is_empty())
            .ok_or_else(invalid)?;
        let target = match kind {
            Some("lib") => Target::Lib(name.to_owned()),
            Some("bin") => Target::Bin(name.to_owned()),
            Some("test") if mode == Mode::Test => Target::Test(name.to_owned()),
            _ => return Err(invalid().into()),
        };
        Ok(Some(Self {
            package: package.parent()?.ok_or_else(invalid)?,
            mode,
            target,
        }))
    }

    /// Construct one direct-child virtual package without allowing path separators.
    pub fn package_label(&self) -> bsmr_error::Result<PackageLabel> {
        FileName::new(self.target.name())?;
        let leaf = self.target.child_name(self.mode);
        if self.target.name().is_empty()
            || (matches!(self.target, Target::Test(_)) && self.mode != Mode::Test)
        {
            return Err(EntryError::Invalid(self.package).into());
        }
        FileName::new(&leaf)?;
        self.package.join(ForwardRelativePath::new(&leaf)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_preserve_target_names_and_operation() {
        for mode in [Mode::Build, Mode::Test] {
            for target in [
                Target::Lib("entry_lib-with-hyphen".into()),
                Target::Bin("entry_bin-with-hyphen".into()),
            ] {
                let entry = Entry {
                    package: PackageLabel::testing_parse("root//crates/app"),
                    mode,
                    target,
                };
                let package = entry.package_label().unwrap();
                assert_eq!(package.parent().unwrap(), Some(entry.package));
                assert_eq!(Entry::parse(package).unwrap(), Some(entry));
            }
        }
    }

    #[test]
    fn reserved_descriptors_fail_closed() {
        assert!(
            Entry::parse(PackageLabel::testing_parse("root//app"))
                .unwrap()
                .is_none()
        );
        for leaf in [
            "__bsmr_cargo_",
            "__bsmr_cargo_check_lib_app",
            "__bsmr_cargo_build_example_app",
            "__bsmr_cargo_test_bin_",
            "__bsmr_cargo_build_test_integration",
        ] {
            assert!(
                Entry::parse(PackageLabel::testing_parse(&format!("root//app/{leaf}"))).is_err()
            );
        }
        for name in ["", "nested/name"] {
            let entry = Entry {
                package: PackageLabel::testing_parse("root//app"),
                mode: Mode::Build,
                target: Target::Bin(name.into()),
            };
            assert!(entry.package_label().is_err());
        }
    }

    #[test]
    fn integration_tests_have_only_test_entrypoints() {
        let mut entry = Entry {
            package: PackageLabel::testing_parse("root//app"),
            mode: Mode::Test,
            target: Target::Test("integration_test".into()),
        };
        assert_eq!(
            Entry::parse(entry.package_label().unwrap()).unwrap(),
            Some(entry.clone())
        );
        entry.mode = Mode::Build;
        assert!(entry.package_label().is_err());
    }
}

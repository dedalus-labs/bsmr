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

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_common::cargo_workspace::parse_rust_toolchain;
use bsmr_common::cargo_workspace::render_cargo_build_file;
use bsmr_common::cargo_workspace::select_rust_toolchain_file;
use bsmr_common::dice::cells::HasCellResolver;
use bsmr_common::dice::cycles::CycleGuard;
use bsmr_common::file_ops::dice::DiceFileComputations;
use bsmr_common::file_ops::error::FileReadErrorContext;
use bsmr_common::legacy_configs::dice::HasLegacyConfigs;
use bsmr_common::legacy_configs::dice::OpaqueLegacyBsmrConfigOnDice;
use bsmr_common::package_boundary::HasPackageBoundaryExceptions;
use bsmr_common::package_listing::PackageBuildSource;
use bsmr_common::package_listing::dice::DicePackageListingResolver;
use bsmr_common::package_listing::listing::PackageListing;
use bsmr_common::pnpm_workspace::HasPnpmWorkspaceGraph;
use bsmr_common::pnpm_workspace::is_native_pnpm_workspace;
use bsmr_common::pnpm_workspace::render_typescript_build_file;
use bsmr_common::python_lock::PylockToml;
use bsmr_common::python_project::PythonRootFiles;
use bsmr_common::python_project::PythonVcsFiles;
use bsmr_common::python_project::PythonWorkspaceMember;
use bsmr_common::python_project::python_project_name;
use bsmr_common::python_project::python_project_uses_vcs;
use bsmr_common::python_project::python_test_locks;
use bsmr_common::python_project::python_workspace_closure;
use bsmr_common::python_project::python_workspace_manifest_paths;
use bsmr_common::python_project::python_workspace_member;
use bsmr_common::python_project::render_python_build_file;
use bsmr_common::python_project::validate_python_build_requirements;
use bsmr_core::build_file_path::BuildFilePath;
use bsmr_core::cells::build_file_cell::BuildFileCell;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;
use bsmr_events::dispatch::span;
use bsmr_events::dispatch::span_async_simple;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use bsmr_interpreter::allow_relative_paths::HasAllowRelativePaths;
use bsmr_interpreter::dice::starlark_provider::StarlarkEvalKind;
use bsmr_interpreter::factory::StarlarkEvaluatorProvider;
use bsmr_interpreter::file_loader::LoadedModule;
use bsmr_interpreter::file_loader::ModuleDeps;
use bsmr_interpreter::from_freeze::from_freeze_error;
use bsmr_interpreter::import_paths::HasImportPaths;
use bsmr_interpreter::load_module::InterpreterCalculation;
use bsmr_interpreter::paths::module::OwnedStarlarkModulePath;
use bsmr_interpreter::paths::module::StarlarkModulePath;
use bsmr_interpreter::paths::package::PackageFilePath;
use bsmr_interpreter::paths::path::OwnedStarlarkPath;
use bsmr_interpreter::paths::path::StarlarkPath;
use bsmr_node::nodes::eval_result::EvaluationResult;
use bsmr_node::super_package::SuperPackage;
use bsmr_util::time_span::TimeSpan;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use pagable::Pagable;
use pagable::pagable_typetag;
use starlark::codemap::FileSpan;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::values::FrozenHeapName;

use crate::interpreter::bsmrconfig::ConfigsOnDiceViewForStarlark;
use crate::interpreter::cell_info::InterpreterCellInfo;
use crate::interpreter::check_starlark_stack_size::check_starlark_stack_size;
use crate::interpreter::cycles::LoadCycleDescriptor;
use crate::interpreter::global_interpreter_state::HasGlobalInterpreterState;
use crate::interpreter::interpreter_for_dir::InterpreterForDir;
use crate::interpreter::interpreter_for_dir::ParseData;
use crate::interpreter::interpreter_for_dir::ParseResult;
use crate::super_package::package_value::SuperPackageValuesImpl;

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum NativeBuildFileError {
    #[error("native Cargo builds require Cargo.toml at the BSMR project root")]
    CargoWorkspaceManifestRequired,
    #[error("native Python builds require pylock.toml at the BSMR project root")]
    PythonRuntimeLockRequired,
    #[error("native Python builds require pylock.build.toml at the BSMR project root")]
    PythonBuildLockRequired,
    #[error("native build source contains none of package.json, Cargo.toml, or pyproject.toml")]
    NoSupportedManifest,
}

fn toml_value_to_json(value: toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => match serde_json::Number::from_f64(f) {
            Some(n) => serde_json::Value::Number(n),
            None => serde_json::Value::Null,
        },
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(table) => serde_json::Value::Object(
            table
                .into_iter()
                .map(|(k, v)| (k, toml_value_to_json(v)))
                .collect(),
        ),
    }
}

#[async_trait]
pub trait HasCalculationDelegate<'c, 'd> {
    /// Get calculator for a file evaluation.
    ///
    /// This function only accepts cell names, but it is created
    /// per evaluated file (build file or `.bzl`).
    async fn get_interpreter_calculator(
        &'c mut self,
        path: OwnedStarlarkPath,
    ) -> bsmr_error::Result<DiceCalculationDelegate<'c, 'd>>;
}

#[async_trait]
impl<'c, 'd> HasCalculationDelegate<'c, 'd> for DiceComputations<'d> {
    async fn get_interpreter_calculator(
        &'c mut self,
        path: OwnedStarlarkPath,
    ) -> bsmr_error::Result<DiceCalculationDelegate<'c, 'd>> {
        #[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
        #[display("{}@{}", _0, _1)]
        #[pagable_typetag(dice::DiceKeyDyn)]
        struct InterpreterConfigForDirKey(CellPath, BuildFileCell);

        #[async_trait]
        impl Key for InterpreterConfigForDirKey {
            type Value = bsmr_error::Result<Arc<InterpreterForDir>>;
            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                _cancellation: &CancellationContext,
            ) -> Self::Value {
                let global_state = ctx.get_global_interpreter_state().await?;

                let cell_alias_resolver = ctx.get_cell_alias_resolver(self.0.cell()).await?;

                let implicit_import_paths = ctx.import_paths_for_cell(self.1).await?;

                let dirs_allowing_relative_paths =
                    ctx.dirs_allowing_relative_paths(self.0.clone()).await?;

                let cell_info = InterpreterCellInfo::new(
                    self.1,
                    ctx.get_cell_resolver().await?,
                    cell_alias_resolver,
                )?;

                Ok(Arc::new(InterpreterForDir::new(
                    cell_info,
                    global_state.dupe(),
                    implicit_import_paths,
                    dirs_allowing_relative_paths,
                )?))
            }

            fn equality(_: &Self::Value, _: &Self::Value) -> bool {
                false
            }

            fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
                OkPagableValueSerialize::<Self::Value>::new()
            }
        }

        let build_file_cell = path.borrow().build_file_cell();
        let configs = self
            .compute(&InterpreterConfigForDirKey(
                path.borrow()
                    .path()
                    .parent()
                    .expect("starlark path to have parent")
                    .to_owned(),
                build_file_cell,
            ))
            .await??;

        Ok(DiceCalculationDelegate {
            build_file_cell,
            ctx: self,
            configs,
        })
    }
}

pub struct DiceCalculationDelegate<'c, 'd> {
    build_file_cell: BuildFileCell,
    ctx: &'c mut DiceComputations<'d>,
    configs: Arc<InterpreterForDir>,
}

impl<'c, 'd: 'c> DiceCalculationDelegate<'c, 'd> {
    async fn get_legacy_bsmr_config_for_starlark(
        &mut self,
    ) -> bsmr_error::Result<OpaqueLegacyBsmrConfigOnDice> {
        self.ctx
            .get_legacy_config_on_dice(self.build_file_cell.name())
            .await
    }

    async fn parse_file(
        &mut self,
        starlark_path: StarlarkPath<'_>,
    ) -> bsmr_error::Result<ParseResult> {
        let result =
            DiceFileComputations::read_file(self.ctx, starlark_path.path().as_ref().as_ref()).await;
        let content = match starlark_path {
            StarlarkPath::BuildFile(file) => {
                result.with_package_context_information(file.path().path().to_string())
            }
            // Should potentially add support for other file types as well
            _ => result.without_package_context_information(),
        }?;

        self.configs.parse(starlark_path, content)
    }

    async fn eval_deps(
        ctx: &mut DiceComputations<'_>,
        modules: &[(Option<FileSpan>, OwnedStarlarkModulePath)],
    ) -> bsmr_error::Result<ModuleDeps> {
        Ok(ModuleDeps(
            ctx.try_compute_join(modules, |ctx, (span, import)| {
                async move {
                    ctx.get_loaded_module(import.borrow())
                        .await
                        .with_buck_error_context(|| {
                            format!(
                                "From load at {}",
                                span.as_ref()
                                    .map_or("implicit location".to_owned(), |file_span| file_span
                                        .resolve()
                                        .begin_file_line()
                                        .to_string())
                            )
                        })
                }
                .boxed()
            })
            .await?,
        ))
    }

    pub async fn prepare_eval(
        &mut self,
        starlark_file: StarlarkPath<'_>,
    ) -> bsmr_error::Result<(AstModule, ModuleDeps)> {
        let ParseData(ast, imports) = self.parse_file(starlark_file).await??;
        let deps = CycleGuard::<LoadCycleDescriptor>::new(self.ctx)?
            .guard_this(Self::eval_deps(self.ctx, &imports))
            .await
            .into_result(self.ctx)
            .await???;
        Ok((ast, deps))
    }

    /// Parses either an explicit Starlark file or a native ecosystem manifest.
    pub(super) async fn prepare_build_file_eval(
        &mut self,
        package: PackageLabel,
        listing: &PackageListing,
    ) -> bsmr_error::Result<(BuildFilePath, AstModule, ModuleDeps)> {
        let build_file_path = BuildFilePath::new(package.dupe(), listing.buildfile().to_owned());
        let (ast, deps) = match listing.build_source() {
            PackageBuildSource::Starlark => {
                self.prepare_eval(StarlarkPath::BuildFile(&build_file_path))
                    .await?
            }
            PackageBuildSource::Native => {
                let mut source = String::new();
                let mut handled_manifest = false;
                if listing
                    .get_file(PackageRelativePath::new("package.json")?)
                    .is_some()
                    && self.root_is_pnpm_workspace(package).await?
                {
                    handled_manifest = true;
                    let graph = self
                        .ctx
                        .get_pnpm_workspace_graph(package.cell_name())
                        .await?;
                    if let Some(typescript) = render_typescript_build_file(
                        &graph,
                        package.as_cell_path().path().to_owned(),
                        listing,
                    )? {
                        source.push_str(&typescript);
                    }
                }
                if listing
                    .get_file(PackageRelativePath::new("Cargo.toml")?)
                    .is_some()
                {
                    handled_manifest = true;
                    source.push_str(&self.render_native_cargo(package).await?);
                }
                if listing
                    .get_file(PackageRelativePath::new("pyproject.toml")?)
                    .is_some()
                {
                    let manifest = self.read_package_file(package, "pyproject.toml").await?;
                    if let Some(python) = self
                        .render_native_python(package, listing, &manifest)
                        .await?
                    {
                        handled_manifest = true;
                        source.push_str(&python);
                    }
                }
                if !handled_manifest {
                    return Err(NativeBuildFileError::NoSupportedManifest.into());
                }
                let ParseData(ast, imports) = self.prepare_eval_with_content(
                    StarlarkPath::BuildFile(&build_file_path),
                    source,
                )??;
                let deps = CycleGuard::<LoadCycleDescriptor>::new(self.ctx)?
                    .guard_this(Self::eval_deps(self.ctx, &imports))
                    .await
                    .into_result(self.ctx)
                    .await???;
                (ast, deps)
            }
        };
        Ok((build_file_path, ast, deps))
    }

    /// Renders one Cargo manifest against the project root's shared workspace inputs.
    async fn render_native_cargo(&mut self, package: PackageLabel) -> bsmr_error::Result<String> {
        let root_path = CellRelativePathBuf::unchecked_new(String::new());
        let root = PackageLabel::new(package.cell_name(), &root_path)?;
        let workspace_listing = DicePackageListingResolver(self.ctx)
            .resolve_package_listing(root)
            .await?;
        if workspace_listing
            .get_file(PackageRelativePath::new("Cargo.toml")?)
            .is_none()
        {
            return Err(NativeBuildFileError::CargoWorkspaceManifestRequired.into());
        }
        let manifest = self.read_package_file(package, "Cargo.toml").await?;
        let toolchain_file = select_rust_toolchain_file(&workspace_listing)?;
        let toolchain = self.read_package_file(root, toolchain_file).await?;
        let toolchain = parse_rust_toolchain(&toolchain)?;
        Ok(render_cargo_build_file(
            package.cell_relative_path().to_owned(),
            &manifest,
            &workspace_listing,
            &toolchain,
        )?)
    }

    /// Validates root Python inputs and renders one project or workspace root.
    async fn render_native_python(
        &mut self,
        package: PackageLabel,
        listing: &PackageListing,
        manifest: &str,
    ) -> bsmr_error::Result<Option<String>> {
        let is_project = python_project_name(manifest)?.is_some();
        let root_path = CellRelativePathBuf::unchecked_new(String::new());
        let root = PackageLabel::new(package.cell_name(), &root_path)?;
        let workspace_listing = DicePackageListingResolver(self.ctx)
            .resolve_package_listing(root)
            .await?;
        let (members, workspace_uses_vcs) = self
            .python_workspace_members(root, &workspace_listing)
            .await?;
        if !is_project && (!package.cell_relative_path().is_empty() || members.is_empty()) {
            return Ok(None);
        }
        if workspace_listing
            .get_file(PackageRelativePath::new("pylock.toml")?)
            .is_none()
        {
            return Err(NativeBuildFileError::PythonRuntimeLockRequired.into());
        }
        let lock = self.read_package_file(root, "pylock.toml").await?;
        let lock = PylockToml::parse(&lock)?;
        let runtime_packages = lock.installation_fragments()?;
        if workspace_listing
            .get_file(PackageRelativePath::new("pylock.build.toml")?)
            .is_none()
        {
            return Err(NativeBuildFileError::PythonBuildLockRequired.into());
        }
        let build_lock = self.read_package_file(root, "pylock.build.toml").await?;
        let build_lock = PylockToml::parse(&build_lock)?;
        let build_packages = build_lock.installation_fragments()?;
        let vcs = if workspace_uses_vcs || python_project_uses_vcs(manifest)? {
            self.python_vcs_files(root).await?
        } else {
            None
        };
        let mut test_locks = python_test_locks(&workspace_listing)?;
        for test_lock in &mut test_locks {
            let lock = self.read_package_file(root, &test_lock.file).await?;
            test_lock.packages = PylockToml::parse(&lock)?.installation_fragments()?;
        }
        let members = python_workspace_closure(manifest, &members)?;
        validate_python_build_requirements(manifest, &members, &lock, &build_lock)?;
        Ok(Some(render_python_build_file(
            package.cell_relative_path().to_owned(),
            manifest,
            listing,
            &PythonRootFiles {
                runtime_packages,
                build_packages,
                members,
                test_locks,
                vcs,
            },
        )?))
    }

    /// Maps nested standard projects to their generated first-party wheel labels.
    async fn python_workspace_members(
        &mut self,
        root: PackageLabel,
        listing: &PackageListing,
    ) -> bsmr_error::Result<(Vec<PythonWorkspaceMember>, bool)> {
        let mut members = Vec::new();
        let mut uses_vcs = false;
        let root_manifest = self.read_package_file(root, "pyproject.toml").await?;
        for file in python_workspace_manifest_paths(&root_manifest, listing)? {
            let manifest = self.read_package_file(root, file).await?;
            uses_vcs |= python_project_uses_vcs(&manifest)?;
            let Some(package) = file.strip_suffix("/pyproject.toml") else {
                return Err(internal_error!(
                    "filtered Python workspace manifest `{file}` has no manifest suffix"
                ));
            };
            let Some(member) = python_workspace_member(package.to_owned(), &manifest)? else {
                continue;
            };
            members.push(member);
        }
        Ok((members, uses_vcs))
    }

    /// Discovers only the Git database components consumed by read-only version queries.
    async fn python_vcs_files(
        &mut self,
        root: PackageLabel,
    ) -> bsmr_error::Result<Option<PythonVcsFiles>> {
        let path = |file: &str| {
            root.to_cell_path()
                .join(ForwardRelativePath::new(file).expect("static Git path is valid"))
        };
        if DiceFileComputations::read_file_if_exists(self.ctx, path(".git/HEAD").as_ref())
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let packed_refs = path(".git/packed-refs");
        let packed_refs = DiceFileComputations::read_file_if_exists(self.ctx, packed_refs.as_ref())
            .await?
            .is_some();
        let shallow = path(".git/shallow");
        let shallow = DiceFileComputations::read_file_if_exists(self.ctx, shallow.as_ref())
            .await?
            .is_some();
        Ok(Some(PythonVcsFiles {
            packed_refs,
            shallow,
        }))
    }

    /// Tests the root pnpm contract without promoting incidental package metadata.
    async fn root_is_pnpm_workspace(&mut self, package: PackageLabel) -> bsmr_error::Result<bool> {
        let root_path = CellRelativePathBuf::unchecked_new(String::new());
        let root = PackageLabel::new(package.cell_name(), &root_path)?;
        let listing = DicePackageListingResolver(self.ctx)
            .resolve_package_listing(root)
            .await?;
        Ok(is_native_pnpm_workspace(&listing))
    }

    /// Reads one package-relative source through DICE so edits invalidate analysis.
    async fn read_package_file(
        &mut self,
        package: PackageLabel,
        file: &str,
    ) -> bsmr_error::Result<String> {
        let path = package.to_cell_path().join(ForwardRelativePath::new(file)?);
        DiceFileComputations::read_file(self.ctx, path.as_ref())
            .await
            .with_package_context_information(path.to_string())
    }

    pub fn prepare_eval_with_content(
        &self,
        starlark_file: StarlarkPath<'_>,
        content: String,
    ) -> bsmr_error::Result<ParseResult> {
        self.configs.parse(starlark_file, content)
    }

    pub async fn resolve_load(
        &self,
        starlark_file: StarlarkPath<'_>,
        load_string: &str,
    ) -> bsmr_error::Result<OwnedStarlarkModulePath> {
        self.configs.resolve_path(starlark_file, load_string)
    }

    pub async fn eval_module_uncached(
        &mut self,
        starlark_file: StarlarkModulePath<'_>,
        cancellation: &CancellationContext,
    ) -> bsmr_error::Result<LoadedModule> {
        match starlark_file {
            StarlarkModulePath::JsonFile(_) => self.eval_json_module_uncached(starlark_file).await,
            StarlarkModulePath::TomlFile(_) => self.eval_toml_file_uncached(starlark_file).await,
            _ => {
                self.eval_starlark_module_uncached(starlark_file, cancellation)
                    .await
            }
        }
    }

    async fn eval_json_module_uncached(
        &mut self,
        starlark_file: StarlarkModulePath<'_>,
    ) -> bsmr_error::Result<LoadedModule> {
        let path = starlark_file.path();
        let contents = DiceFileComputations::read_file(self.ctx, path.as_ref())
            .await
            .with_package_context_information(path.path().to_string())?;

        let value: serde_json::Value = serde_json::from_str(&contents)
            .with_buck_error_context(|| format!("Parsing {path}"))?;

        // patternlint-disable-next-line bsmr-no-starlark-module: We expect these to be small + simple
        let frozen = Module::with_temp_heap(|module| {
            module.set("value", module.heap().alloc(value));
            module
                .freeze_named(FrozenHeapName::User(Box::new(StarlarkEvalKind::Load(
                    Arc::new(OwnedStarlarkModulePath::new(starlark_file)),
                ))))
                .map_err(from_freeze_error)
        })?;
        Ok(LoadedModule::new(
            OwnedStarlarkModulePath::new(starlark_file),
            Default::default(),
            frozen,
        ))
    }

    async fn eval_toml_file_uncached(
        &mut self,
        starlark_file: StarlarkModulePath<'_>,
    ) -> bsmr_error::Result<LoadedModule> {
        let path = starlark_file.path();
        let contents = DiceFileComputations::read_file(self.ctx, path.as_ref())
            .await
            .with_package_context_information(path.path().to_string())?;

        let value: toml::Value =
            toml::from_str(&contents).with_buck_error_context(|| format!("Parsing {path}"))?;
        let json_value = toml_value_to_json(value);

        // patternlint-disable-next-line bsmr-no-starlark-module: We expect these to be small + simple
        let frozen = Module::with_temp_heap(|module| {
            module.set("value", module.heap().alloc(json_value));
            module
                .freeze_named(FrozenHeapName::User(Box::new(StarlarkEvalKind::Load(
                    Arc::new(OwnedStarlarkModulePath::new(starlark_file)),
                ))))
                .map_err(from_freeze_error)
        })?;
        Ok(LoadedModule::new(
            OwnedStarlarkModulePath::new(starlark_file),
            Default::default(),
            frozen,
        ))
    }

    async fn eval_starlark_module_uncached(
        &mut self,
        starlark_file: StarlarkModulePath<'_>,
        cancellation: &CancellationContext,
    ) -> bsmr_error::Result<LoadedModule> {
        let (ast, deps) = self.prepare_eval(starlark_file.into()).await?;
        let loaded_modules = deps.get_loaded_modules();
        let bsmrconfig = self.get_legacy_bsmr_config_for_starlark().await?;
        let root_bsmrconfig = self.ctx.get_legacy_root_config_on_dice().await?;

        let configs = &self.configs;
        let ctx = &mut *self.ctx;

        let eval_kind = StarlarkEvalKind::Load(Arc::new(starlark_file.to_owned()));
        let provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;

        let mut bsmrconfigs = ConfigsOnDiceViewForStarlark::new(ctx, bsmrconfig, root_bsmrconfig);
        let evaluation = configs
            .eval_module(
                starlark_file,
                &mut bsmrconfigs,
                ast,
                loaded_modules.clone(),
                provider,
                cancellation,
            )
            .with_buck_error_context(|| format!("Error evaluating module: `{}`", starlark_file))?;

        Ok(LoadedModule::new(
            OwnedStarlarkModulePath::new(starlark_file),
            loaded_modules,
            evaluation,
        ))
    }

    /// Eval parent `PACKAGE` file for given package file.
    async fn eval_parent_package_file(
        &mut self,
        file: PackageLabel,
    ) -> bsmr_error::Result<SuperPackage> {
        let cell_resolver = self.ctx.get_cell_resolver().await?;
        let proj_rel_path = cell_resolver.resolve_path(file.as_cell_path())?;
        match proj_rel_path.parent() {
            None => {
                // We are in the project root, there's no parent.
                Ok(SuperPackage::empty::<SuperPackageValuesImpl>()?)
            }
            Some(parent) => {
                let parent_cell = cell_resolver.get_cell_path(parent);
                self.eval_package_file(PackageLabel::from_cell_path(parent_cell.as_ref())?)
                    .await
            }
        }
    }

    /// Return `None` if there's no `PACKAGE` file in the directory.
    pub async fn prepare_package_file_eval(
        &mut self,
        package: PackageLabel,
    ) -> bsmr_error::Result<Option<(PackageFilePath, AstModule, ModuleDeps)>> {
        // Note:
        /// To avoid paying the cost of read_dir when computing if any specific file has changed (e.g. PACKAGE),
        /// we depend on directory_sublisting_matching_any_case_key to invalidate all files that match (regardless of case).
        /// We need to do this to make sure to work with case-sensitive file paths.
        //   * `read_path_metadata` would not tell us if the file name is `PACKAGE`
        //     and not `package` on case-insensitive filesystems.
        //     We do case-sensitive comparison for `BUILD.bsmr` files, so we do the same here.
        //   * we fail here if `PACKAGE` (but not `package`) exists, and it is not a file.

        // package file results capture starlark values and so cannot be checked for equality. This means we
        // can't get early cutoff for the consumers, and so we need to be careful to ensure our deps are precise.
        // Otherwise noop package value recomputations can lead to large recompute costs.
        //
        // Here we put the package file check behind an additional dice key so that we don't recompute on irrelevant
        // changes to the directory contents.
        #[derive(Debug, Display, Clone, Allocative, Eq, PartialEq, Hash, Pagable)]
        #[pagable_typetag(dice::DiceKeyDyn)]
        struct PackageFileLookupKey(PackageLabel);

        #[async_trait]
        impl Key for PackageFileLookupKey {
            type Value = bsmr_error::Result<Option<Arc<PackageFilePath>>>;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                _cancellation: &CancellationContext,
            ) -> Self::Value {
                for package_file_path in PackageFilePath::for_dir(self.0.as_cell_path()) {
                    if DiceFileComputations::exists_matching_exact_case(
                        ctx,
                        package_file_path.path().as_ref(),
                    )
                    .await?
                    {
                        return Ok(Some(Arc::new(package_file_path)));
                    }
                }
                Ok(None)
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                match (x, y) {
                    (Ok(x), Ok(y)) => x == y,
                    _ => false,
                }
            }

            fn validity(x: &Self::Value) -> bool {
                x.is_ok()
            }

            fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
                OkPagableValueSerialize::<Self::Value>::new()
            }
        }

        match self
            .ctx
            .compute(&PackageFileLookupKey(package.dupe()))
            .await??
        {
            Some(package_file_path) => {
                let (module, deps) = self
                    .prepare_eval(StarlarkPath::PackageFile(&package_file_path))
                    .await?;
                Ok(Some(((*package_file_path).clone(), module, deps)))
            }
            None => Ok(None),
        }
    }

    async fn eval_package_file_uncached(
        &mut self,
        path: PackageLabel,
        cancellation: &CancellationContext,
    ) -> bsmr_error::Result<SuperPackage> {
        let parent = self.eval_parent_package_file(path.dupe()).await?;
        let ast_deps = self.prepare_package_file_eval(path.dupe()).await?;

        let (package_file_path, ast, deps) = match ast_deps {
            Some(x) => x,
            None => {
                // If there's no `PACKAGE` file, return parent.
                return Ok(parent);
            }
        };

        let bsmrconfig = self.get_legacy_bsmr_config_for_starlark().await?;
        let root_bsmrconfig = self.ctx.get_legacy_root_config_on_dice().await?;

        let configs = &self.configs;
        let ctx = &mut *self.ctx;

        let eval_kind = StarlarkEvalKind::LoadPackageFile(path.dupe());
        let provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;

        let mut bsmrconfigs = ConfigsOnDiceViewForStarlark::new(ctx, bsmrconfig, root_bsmrconfig);

        configs
            .eval_package_file(
                &package_file_path,
                ast,
                parent,
                &mut bsmrconfigs,
                deps.get_loaded_modules(),
                provider,
                cancellation,
            )
            .with_buck_error_context(|| format!("evaluating Starlark PACKAGE file `{path}`"))
    }

    pub(crate) async fn eval_package_file(
        &mut self,
        path: PackageLabel,
    ) -> bsmr_error::Result<SuperPackage> {
        #[derive(Debug, Display, Clone, Allocative, Eq, PartialEq, Hash, Pagable)]
        #[pagable_typetag(dice::DiceKeyDyn)]
        struct PackageFileKey(PackageLabel);

        #[async_trait]
        impl Key for PackageFileKey {
            type Value = bsmr_error::Result<SuperPackage>;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                cancellation: &CancellationContext,
            ) -> Self::Value {
                let mut interpreter = ctx
                    .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                        PackageFilePath::package_file_for_dir(self.0.as_cell_path()),
                    ))
                    .await?;
                interpreter
                    .eval_package_file_uncached(self.0.dupe(), cancellation)
                    .await
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                match (x, y) {
                    (Ok(x), Ok(y)) => x == y,
                    _ => false,
                }
            }

            fn validity(x: &Self::Value) -> bool {
                x.is_ok()
            }

            fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
                OkPagableValueSerialize::<Self::Value>::new()
            }
        }

        self.ctx.compute(&PackageFileKey(path)).await?
    }

    /// Most directories do not contain a `PACKAGE` file, this function
    /// optimizes `eval_package_file` for this case by avoiding creation of DICE key.
    pub(crate) async fn eval_package_file_for_build_file(
        &mut self,
        package: PackageLabel,
        package_listing: &PackageListing,
    ) -> bsmr_error::Result<SuperPackage> {
        for package_file_name in PackageFilePath::package_file_names() {
            if package_listing
                .get_file(PackageRelativePath::new(package_file_name)?)
                .is_some()
            {
                return self.eval_package_file(package).await;
            }
        }

        // Without this optimization, `cquery <that android target>` has 6% time regression.
        // With this optimization, check for `PACKAGE` files adds 2% to time.
        self.eval_parent_package_file(package).await
    }

    async fn resolve_package_listing(
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> bsmr_error::Result<PackageListing> {
        span_async_simple(
            bsmr_data::LoadPackageStart {
                path: package.as_cell_path().to_string(),
            },
            DicePackageListingResolver(ctx).resolve_package_listing(package.dupe()),
            bsmr_data::LoadPackageEnd {
                path: package.as_cell_path().to_string(),
            },
        )
        .await
    }

    pub async fn eval_build_file(
        &mut self,
        package: PackageLabel,
        cancellation: &CancellationContext,
    ) -> (TimeSpan, bsmr_error::Result<Arc<EvaluationResult>>) {
        let mut now = None;
        let eval_kind = StarlarkEvalKind::LoadBuildFile(package.dupe());
        let eval_result: bsmr_error::Result<_> = try {
            let ((), listing) = self
                .ctx
                .try_compute2(
                    |ctx| check_starlark_stack_size(ctx).boxed(),
                    |ctx| Self::resolve_package_listing(ctx, package.dupe()).boxed(),
                )
                .await?;

            let (build_file_path, ast, deps) = self
                .prepare_build_file_eval(package.dupe(), &listing)
                .await?;
            let super_package = self
                .eval_package_file_for_build_file(package.dupe(), &listing)
                .await?;

            let package_boundary_exception = self
                .ctx
                .get_package_boundary_exception(package.as_cell_path())
                .await?
                .is_some();
            let bsmrconfig = self.get_legacy_bsmr_config_for_starlark().await?;
            let root_bsmrconfig = self.ctx.get_legacy_root_config_on_dice().await?;
            let module_id = build_file_path.to_string();
            let cell_str = build_file_path.cell().as_str().to_owned();
            let start_event = bsmr_data::LoadBuildFileStart {
                cell: cell_str.clone(),
                module_id: module_id.clone(),
            };

            let configs = &self.configs;
            let ctx = &mut *self.ctx;

            now = Some(TimeSpan::start_now());
            let provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;
            let mut bsmrconfigs =
                ConfigsOnDiceViewForStarlark::new(ctx, bsmrconfig, root_bsmrconfig);

            let (profile_data, eval_result) = span(start_event, move || {
                let result_with_stats = configs
                    .eval_build_file(
                        &build_file_path,
                        &mut bsmrconfigs,
                        listing,
                        super_package,
                        package_boundary_exception,
                        ast,
                        deps.get_loaded_modules(),
                        provider,
                        false,
                        cancellation,
                    )
                    .with_buck_error_context(|| {
                        format!("Error evaluating build file: `{}`", build_file_path)
                    });
                let error = result_with_stats.as_ref().err().map(|e| format!("{e:#}"));
                let (
                    starlark_peak_allocated_bytes,
                    cpu_instruction_count,
                    starlark_tick_count,
                    target_count,
                ) = match &result_with_stats {
                    Ok((_, rs)) => (
                        Some(rs.starlark_peak_allocated_bytes),
                        rs.cpu_instruction_count,
                        Some(rs.starlark_tick_count),
                        Some(rs.result.targets().len() as u64),
                    ),
                    Err(_) => (None, None, None, None),
                };

                (
                    result_with_stats,
                    bsmr_data::LoadBuildFileEnd {
                        module_id,
                        cell: cell_str,
                        target_count,
                        starlark_peak_allocated_bytes,
                        cpu_instruction_count,
                        error,
                        starlark_tick_count,
                    },
                )
            })?;

            let mut eval_result = eval_result.result;

            if eval_result.starlark_profile.is_some() {
                return (
                    now.unwrap().end_now(),
                    Err(internal_error!(
                        "starlark_profile field must not be set yet"
                    )),
                );
            }
            eval_result.starlark_profile = profile_data.map(|d| d as _);
            eval_result
        };

        (
            now.map_or(TimeSpan::empty_now(), |v| v.end_now()),
            eval_result.map(Arc::new),
        )
    }
}

mod keys {
    use allocative::Allocative;
    use bsmr_interpreter::paths::module::OwnedStarlarkModulePath;
    use derive_more::Display;
    use pagable::Pagable;
    use pagable::pagable_typetag;

    #[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
    #[pagable_typetag(dice::DiceKeyDyn)]
    pub struct EvalImportKey(pub OwnedStarlarkModulePath);
}

pub mod testing {
    // re-exports for testing
    pub use super::keys::EvalImportKey;
}

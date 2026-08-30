use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use syn::{Expr, ExprLit, Item, Lit, Meta, UseTree};

use crate::architecture_tests::model::{
    ArchitectureAuditError, CanonicalDependency, CanonicalOrigin, CargoDependencyAuthority,
    CargoDependencyDeclaration, CargoDependencyScope, ExternalDependencyOrigin, ProductionRoot,
    ProductionRootKind, RawDependency, RustDependencyMode, RustWorkspaceModel,
};
use crate::test_support::source_audit::{is_test_only, normalized_ident};

use super::DependencyResolutionFailure;

pub(super) fn resolve_canonical_dependencies(
    workspace: &RustWorkspaceModel,
    raw: &[RawDependency],
) -> Result<Vec<CanonicalDependency>, ArchitectureAuditError> {
    resolve_canonical_dependencies_detailed(workspace, raw).map_err(|failure| failure.error)
}

pub(super) fn resolve_canonical_dependencies_detailed(
    workspace: &RustWorkspaceModel,
    raw: &[RawDependency],
) -> Result<Vec<CanonicalDependency>, DependencyResolutionFailure> {
    let mut resolver = Resolver::new(workspace);
    let mut canonical = Vec::with_capacity(raw.len());
    for dependency in raw {
        resolver.resolution_steps = 0;
        let origin = resolver.resolve_dependency(dependency).map_err(|error| {
            DependencyResolutionFailure {
                dependency: dependency.clone(),
                error,
            }
        })?;
        let canonical_origin_target = canonical_origin_target(&origin);
        canonical.push(CanonicalDependency {
            owning_package: dependency.owning_package.clone(),
            source_file: dependency.repository_relative_source_file.clone(),
            owner: dependency.fully_qualified_owner.clone(),
            kind: dependency.kind,
            mode: dependency.mode,
            origin,
            canonical_origin_target,
            line: dependency.line,
            column: dependency.column,
        });
    }
    canonical.sort();
    Ok(canonical)
}

fn canonical_origin_target(origin: &CanonicalOrigin) -> String {
    match origin {
        CanonicalOrigin::Repository {
            fully_qualified_target,
            ..
        } => fully_qualified_target.clone(),
        CanonicalOrigin::LanguageBuiltin {
            crate_name,
            canonical_subpath,
        } => canonical_subpath
            .as_ref()
            .map(|subpath| format!("{crate_name}::{subpath}"))
            .unwrap_or_else(|| crate_name.clone()),
        CanonicalOrigin::RepositoryAsset {
            repository_relative_path,
        } => format!("repository-asset:{repository_relative_path}"),
        CanonicalOrigin::External(external) => external
            .canonical_subpath
            .as_ref()
            .map(|subpath| format!("external:{}::{subpath}", external.package_name))
            .unwrap_or_else(|| format!("external:{}", external.package_name)),
    }
}

struct Resolver<'a> {
    workspace: &'a RustWorkspaceModel,
    declarations: BTreeMap<(String, String), Vec<CargoDependencyDeclaration>>,
    active_targets: BTreeSet<RepositoryTargetKey>,
    resolved_targets: BTreeMap<RepositoryTargetKey, CanonicalOrigin>,
    unresolved_targets: BTreeSet<RepositoryTargetKey>,
    active_aliases: BTreeSet<(PathBuf, String, ImportVisibility)>,
    alias_candidates: BTreeMap<(PathBuf, String, ImportVisibility), ImportAliasCandidates>,
    parsed_sources: BTreeMap<PathBuf, Arc<syn::File>>,
    symbol_declarations: BTreeMap<(PathBuf, String), Option<PathBuf>>,
    canonical_repository_root: Option<PathBuf>,
    resolution_steps: usize,
}

type RepositoryTargetKey = (
    String,
    String,
    PathBuf,
    String,
    ImportVisibility,
    RustDependencyMode,
);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ImportVisibility {
    Lexical,
    Exported,
}

impl<'a> Resolver<'a> {
    fn new(workspace: &'a RustWorkspaceModel) -> Self {
        let mut declarations = BTreeMap::<_, Vec<_>>::new();
        for declaration in workspace.dependency_declarations.iter().cloned() {
            declarations
                .entry((
                    declaration.owning_package.clone(),
                    declaration.declared_name.clone(),
                ))
                .or_default()
                .push(declaration);
        }
        Self {
            workspace,
            declarations,
            active_targets: BTreeSet::new(),
            resolved_targets: BTreeMap::new(),
            unresolved_targets: BTreeSet::new(),
            active_aliases: BTreeSet::new(),
            alias_candidates: BTreeMap::new(),
            parsed_sources: BTreeMap::new(),
            symbol_declarations: BTreeMap::new(),
            canonical_repository_root: None,
            resolution_steps: 0,
        }
    }

    fn resolve_dependency(
        &mut self,
        dependency: &RawDependency,
    ) -> Result<CanonicalOrigin, ArchitectureAuditError> {
        if matches!(
            dependency.kind,
            crate::architecture_tests::model::RustDependencyKind::Attribute
                | crate::architecture_tests::model::RustDependencyKind::Include
        ) {
            return self.resolve_repository_asset(dependency);
        }
        let segments = split_target(&dependency.written_target)?;
        let root = root_for_dependency(self.workspace, dependency)?;
        let owner_segments = dependency
            .fully_qualified_owner
            .split("::")
            .skip(1)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let scope_file = self
            .workspace
            .repository_root
            .join(&dependency.repository_relative_source_file);
        self.resolve_segments(
            &dependency.owning_package,
            &root.target.replace('-', "_"),
            &root.source_path,
            &scope_file,
            &owner_segments,
            &segments,
            dependency.mode,
        )
    }

    fn resolve_repository_asset(
        &self,
        dependency: &RawDependency,
    ) -> Result<CanonicalOrigin, ArchitectureAuditError> {
        let repository_root =
            std::fs::canonicalize(&self.workspace.repository_root).map_err(|source| {
                ArchitectureAuditError::Io {
                    path: self.workspace.repository_root.clone(),
                    source,
                }
            })?;
        let source_file = repository_root.join(&dependency.repository_relative_source_file);
        let candidate = source_file
            .parent()
            .ok_or_else(|| ArchitectureAuditError::InvalidDependencyTarget {
                target: dependency.written_target.clone(),
            })?
            .join(&dependency.written_target);
        let canonical =
            std::fs::canonicalize(&candidate).map_err(|source| ArchitectureAuditError::Io {
                path: candidate,
                source,
            })?;
        let relative = canonical.strip_prefix(&repository_root).map_err(|_| {
            ArchitectureAuditError::SourceEscapesRepository {
                path: canonical.clone(),
                repository_root: repository_root.clone(),
            }
        })?;
        Ok(CanonicalOrigin::RepositoryAsset {
            repository_relative_path: relative.to_string_lossy().replace('\\', "/"),
        })
    }

    fn resolve_segments(
        &mut self,
        owning_package: &str,
        root_owner: &str,
        root_path: &Path,
        scope_file: &Path,
        current_module: &[String],
        segments: &[String],
        mode: RustDependencyMode,
    ) -> Result<CanonicalOrigin, ArchitectureAuditError> {
        self.resolution_steps += 1;
        if self.resolution_steps > 1_000 {
            return Err(ArchitectureAuditError::ResolverStepLimit {
                steps: self.resolution_steps,
                source_file: scope_file.to_path_buf(),
                current_module: current_module.join("::"),
                target: segments.join("::"),
            });
        }
        let Some(first) = segments.first() else {
            return Err(ArchitectureAuditError::InvalidDependencyTarget {
                target: String::new(),
            });
        };
        if matches!(first.as_str(), "std" | "core" | "alloc") {
            return Ok(CanonicalOrigin::LanguageBuiltin {
                crate_name: first.clone(),
                canonical_subpath: nonempty_subpath(&segments[1..]),
            });
        }
        if is_primitive_type(first) {
            return Ok(CanonicalOrigin::LanguageBuiltin {
                crate_name: "core".to_owned(),
                canonical_subpath: Some(format!("primitive::{}", segments.join("::"))),
            });
        }
        if first == "crate" {
            return self.resolve_repository_target(
                owning_package,
                root_owner,
                root_path,
                &segments[1..],
                ImportVisibility::Exported,
                mode,
            );
        }
        if matches!(first.as_str(), "self" | "super") {
            let mut repository_segments = current_module.to_vec();
            let mut index = 0;
            if first == "self" {
                index = 1;
            } else {
                while segments
                    .get(index)
                    .is_some_and(|segment| segment == "super")
                {
                    repository_segments.pop();
                    index += 1;
                }
            }
            repository_segments.extend_from_slice(&segments[index..]);
            return self.resolve_repository_target(
                owning_package,
                root_owner,
                root_path,
                &repository_segments,
                ImportVisibility::Lexical,
                mode,
            );
        }
        if let Some(library_root) = self
            .workspace
            .roots
            .iter()
            .find(|root| {
                root.package == owning_package
                    && root.kind == ProductionRootKind::Library
                    && root.target.replace('-', "_") == *first
            })
            .cloned()
        {
            return self.resolve_repository_target(
                &library_root.package,
                &library_root.target.replace('-', "_"),
                &library_root.source_path,
                &segments[1..],
                ImportVisibility::Exported,
                mode,
            );
        }
        if let Some(declarations) = self
            .declarations
            .get(&(owning_package.to_owned(), first.clone()))
        {
            let declaration = Self::declaration_for_mode(declarations, &segments.join("::"), mode)?;
            return match declaration.authority {
                CargoDependencyAuthority::WorkspaceMember { .. } => {
                    let alias = self
                        .workspace
                        .workspace_member_crate_aliases
                        .iter()
                        .find(|alias| {
                            alias.owning_package == owning_package
                                && alias.declared_name == declaration.declared_name
                        })
                        .cloned()
                        .ok_or_else(|| ArchitectureAuditError::UnresolvedWorkspaceMember {
                            target: segments.join("::"),
                        })?;
                    self.resolve_repository_target(
                        &alias.member_package,
                        &alias.library_crate_name,
                        &alias.library_root,
                        &segments[1..],
                        ImportVisibility::Exported,
                        mode,
                    )
                }
                CargoDependencyAuthority::External => {
                    Ok(CanonicalOrigin::External(ExternalDependencyOrigin {
                        declared_name: declaration.declared_name,
                        package_name: declaration.package_name,
                        declaration_scope: declaration.scope,
                        target_condition: declaration.target_condition,
                        canonical_subpath: nonempty_subpath(&segments[1..]),
                    }))
                }
            };
        }
        let mut relative_segments = current_module.to_vec();
        relative_segments.extend_from_slice(segments);
        match self.resolve_repository_target(
            owning_package,
            root_owner,
            root_path,
            &relative_segments,
            ImportVisibility::Exported,
            mode,
        ) {
            Ok(origin) => return Ok(origin),
            Err(ArchitectureAuditError::UnresolvedRepositoryTarget { .. }) => {}
            Err(error) => return Err(error),
        }
        match self.resolve_repository_target(
            owning_package,
            root_owner,
            root_path,
            segments,
            ImportVisibility::Exported,
            mode,
        ) {
            Ok(origin) => return Ok(origin),
            Err(ArchitectureAuditError::UnresolvedRepositoryTarget { .. }) => {}
            Err(error) => return Err(error),
        }
        if let Some(origin) = self.resolve_import_alias(
            owning_package,
            root_owner,
            root_path,
            scope_file,
            current_module,
            first,
            &segments[1..],
            ImportVisibility::Lexical,
            mode,
        )? {
            return Ok(origin);
        }
        Err(ArchitectureAuditError::UnknownDependencyTarget {
            owning_package: owning_package.to_owned(),
            target: segments.join("::"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_import_alias(
        &mut self,
        owning_package: &str,
        root_owner: &str,
        root_path: &Path,
        scope_file: &Path,
        current_module: &[String],
        alias: &str,
        remaining_segments: &[String],
        visibility: ImportVisibility,
        mode: RustDependencyMode,
    ) -> Result<Option<CanonicalOrigin>, ArchitectureAuditError> {
        let scope_file = self.canonical_source_file(scope_file)?;
        let alias_key = (scope_file.clone(), alias.to_owned(), visibility);
        let candidates = self.import_alias_candidates(&scope_file, alias, visibility)?;
        if candidates.paths.is_empty() {
            return Ok(None);
        }
        if !self.active_aliases.insert(alias_key.clone()) {
            return Err(ArchitectureAuditError::ImportAliasCycle {
                source_file: scope_file,
                alias: alias.to_owned(),
                target: std::iter::once(alias)
                    .chain(remaining_segments.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join("::"),
            });
        }
        let result = (|| {
            let mut origins = BTreeSet::new();
            for mut candidate in candidates.paths {
                candidate.extend_from_slice(remaining_segments);
                match self.resolve_segments(
                    owning_package,
                    root_owner,
                    root_path,
                    &scope_file,
                    current_module,
                    &candidate,
                    mode,
                ) {
                    Ok(origin) => {
                        origins.insert(origin);
                    }
                    Err(ArchitectureAuditError::UnresolvedRepositoryTarget { .. })
                    | Err(ArchitectureAuditError::UnknownDependencyTarget { .. }) => {}
                    Err(ArchitectureAuditError::RepositoryTargetCycle { .. })
                    | Err(ArchitectureAuditError::ImportAliasCycle { .. })
                        if candidates.speculative => {}
                    Err(error) => return Err(error),
                }
            }
            match origins.len() {
                0 => Ok(None),
                1 => Ok(origins.pop_first()),
                _ => Err(ArchitectureAuditError::AmbiguousImportAlias {
                    source_file: scope_file,
                    alias: alias.to_owned(),
                }),
            }
        })();
        self.active_aliases.remove(&alias_key);
        result
    }

    fn canonical_repository_root(&mut self) -> Result<PathBuf, ArchitectureAuditError> {
        if let Some(repository_root) = &self.canonical_repository_root {
            return Ok(repository_root.clone());
        }
        let repository_root =
            std::fs::canonicalize(&self.workspace.repository_root).map_err(|source| {
                ArchitectureAuditError::Io {
                    path: self.workspace.repository_root.clone(),
                    source,
                }
            })?;
        self.canonical_repository_root = Some(repository_root.clone());
        Ok(repository_root)
    }

    fn canonical_source_file(&mut self, file: &Path) -> Result<PathBuf, ArchitectureAuditError> {
        let repository_root = self.canonical_repository_root()?;
        canonicalize_repository_source(&repository_root, file)
    }

    fn parsed_source(
        &mut self,
        file: &Path,
    ) -> Result<(PathBuf, Arc<syn::File>), ArchitectureAuditError> {
        let file = self.canonical_source_file(file)?;
        if let Some(syntax) = self.parsed_sources.get(&file) {
            return Ok((file, Arc::clone(syntax)));
        }
        let source =
            std::fs::read_to_string(&file).map_err(|source| ArchitectureAuditError::Io {
                path: file.clone(),
                source,
            })?;
        let syntax = Arc::new(syn::parse_file(&source).map_err(|source| {
            ArchitectureAuditError::SourceParse {
                path: file.clone(),
                source,
            }
        })?);
        self.parsed_sources
            .insert(file.clone(), Arc::clone(&syntax));
        Ok((file, syntax))
    }

    fn import_alias_candidates(
        &mut self,
        file: &Path,
        alias: &str,
        visibility: ImportVisibility,
    ) -> Result<ImportAliasCandidates, ArchitectureAuditError> {
        let file = self.canonical_source_file(file)?;
        let key = (file.clone(), alias.to_owned(), visibility);
        if let Some(candidates) = self.alias_candidates.get(&key) {
            return Ok(candidates.clone());
        }
        let (_, syntax) = self.parsed_source(&file)?;
        let mut matches = Vec::new();
        let mut glob_matches = Vec::new();
        for item in &syntax.items {
            let Item::Use(item_use) = item else {
                continue;
            };
            if is_test_only(&item_use.attrs) {
                continue;
            }
            if visibility == ImportVisibility::Exported
                && matches!(&item_use.vis, syn::Visibility::Inherited)
            {
                continue;
            }
            let mut paths = Vec::new();
            collect_use_targets(&item_use.tree, &mut Vec::new(), &mut paths);
            for (mut path, visible) in paths {
                if visible == alias {
                    matches.push(path);
                } else if visible == "*" {
                    path.pop();
                    path.push(alias.to_owned());
                    glob_matches.push(path);
                }
            }
        }
        matches.sort();
        matches.dedup();
        glob_matches.sort();
        glob_matches.dedup();
        let candidates = if !matches.is_empty() {
            ImportAliasCandidates {
                paths: matches,
                speculative: false,
            }
        } else if self.find_symbol_declaration_file(&file, alias)?.is_some() {
            ImportAliasCandidates {
                paths: Vec::new(),
                speculative: false,
            }
        } else {
            ImportAliasCandidates {
                paths: glob_matches,
                speculative: true,
            }
        };
        self.alias_candidates.insert(key, candidates.clone());
        Ok(candidates)
    }

    fn find_symbol_declaration_file(
        &mut self,
        file: &Path,
        symbol: &str,
    ) -> Result<Option<PathBuf>, ArchitectureAuditError> {
        let file = self.canonical_source_file(file)?;
        let key = (file.clone(), symbol.to_owned());
        if let Some(declaration) = self.symbol_declarations.get(&key) {
            return Ok(declaration.clone());
        }
        let repository_root = self.canonical_repository_root()?;
        let mut active = BTreeSet::new();
        let mut completed = BTreeSet::new();
        let mut declarations = BTreeSet::new();
        self.collect_symbol_declarations(
            &file,
            symbol,
            &repository_root,
            &mut active,
            &mut completed,
            &mut declarations,
        )?;
        let declaration = match declarations.len() {
            0 => None,
            1 => declarations.pop_first(),
            _ => {
                return Err(ArchitectureAuditError::AmbiguousRepositorySymbol {
                    symbol: symbol.to_owned(),
                    declaration_files: declarations.into_iter().collect(),
                });
            }
        };
        self.symbol_declarations.insert(key, declaration.clone());
        Ok(declaration)
    }

    fn collect_symbol_declarations(
        &mut self,
        file: &Path,
        symbol: &str,
        repository_root: &Path,
        active: &mut BTreeSet<PathBuf>,
        completed: &mut BTreeSet<PathBuf>,
        declarations: &mut BTreeSet<PathBuf>,
    ) -> Result<(), ArchitectureAuditError> {
        let (file, syntax) = self.parsed_source(file)?;
        if completed.contains(&file) {
            return Ok(());
        }
        if !active.insert(file.clone()) {
            return Err(ArchitectureAuditError::IncludeCycle { source_file: file });
        }
        let result =
            (|| {
                if syntax
                    .items
                    .iter()
                    .any(|item| item_declares_symbol(item, symbol))
                {
                    declarations.insert(file.clone());
                }
                for item in &syntax.items {
                    let Item::Macro(item_macro) = item else {
                        continue;
                    };
                    if is_test_only(&item_macro.attrs) || !item_macro.mac.path.is_ident("include") {
                        continue;
                    }
                    let target = syn::parse2::<syn::LitStr>(item_macro.mac.tokens.clone())
                        .map_err(|_| ArchitectureAuditError::UnresolvedInclude {
                            source_file: file.clone(),
                            target: item_macro.mac.tokens.to_string(),
                        })?;
                    let included_file = file
                        .parent()
                        .unwrap_or(repository_root)
                        .join(target.value());
                    self.collect_symbol_declarations(
                        &included_file,
                        symbol,
                        repository_root,
                        active,
                        completed,
                        declarations,
                    )?;
                }
                Ok(())
            })();
        active.remove(&file);
        if result.is_ok() {
            completed.insert(file);
        }
        result
    }

    fn validate_scope(
        declaration: &CargoDependencyDeclaration,
        target: &str,
        mode: RustDependencyMode,
    ) -> Result<(), ArchitectureAuditError> {
        match (mode, declaration.scope) {
            (RustDependencyMode::Runtime, CargoDependencyScope::Runtime)
            | (RustDependencyMode::Build, CargoDependencyScope::Build) => Ok(()),
            (_, CargoDependencyScope::Development) => {
                Err(ArchitectureAuditError::DevelopmentDependencyInProduction {
                    target: target.to_owned(),
                })
            }
            _ => Err(ArchitectureAuditError::DependencyScopeMismatch {
                owning_package: declaration.owning_package.clone(),
                target: target.to_owned(),
            }),
        }
    }

    fn declaration_for_mode(
        declarations: &[CargoDependencyDeclaration],
        target: &str,
        mode: RustDependencyMode,
    ) -> Result<CargoDependencyDeclaration, ArchitectureAuditError> {
        let expected_scope = match mode {
            RustDependencyMode::Runtime => CargoDependencyScope::Runtime,
            RustDependencyMode::Build => CargoDependencyScope::Build,
        };
        if let Some(declaration) = declarations
            .iter()
            .find(|declaration| {
                declaration.scope == expected_scope && declaration.target_condition.is_none()
            })
            .or_else(|| {
                declarations
                    .iter()
                    .find(|declaration| declaration.scope == expected_scope)
            })
        {
            return Ok(declaration.clone());
        }

        let fallback = declarations
            .iter()
            .find(|declaration| declaration.scope == CargoDependencyScope::Development)
            .or_else(|| declarations.first())
            .expect("a declaration group must never be empty");
        Self::validate_scope(fallback, target, mode)?;
        unreachable!("a declaration with the requested scope must have returned above")
    }

    fn resolve_repository_target(
        &mut self,
        package: &str,
        root_owner: &str,
        root_path: &Path,
        segments: &[String],
        visibility: ImportVisibility,
        mode: RustDependencyMode,
    ) -> Result<CanonicalOrigin, ArchitectureAuditError> {
        let target = if segments.is_empty() {
            root_owner.to_owned()
        } else {
            format!("{root_owner}::{}", segments.join("::"))
        };
        let key = (
            package.to_owned(),
            root_owner.to_owned(),
            root_path.to_path_buf(),
            target.clone(),
            visibility,
            mode,
        );
        if let Some(origin) = self.resolved_targets.get(&key) {
            return Ok(origin.clone());
        }
        if self.unresolved_targets.contains(&key) {
            return Err(ArchitectureAuditError::UnresolvedRepositoryTarget { target });
        }
        if !self.active_targets.insert(key.clone()) {
            return Err(ArchitectureAuditError::RepositoryTargetCycle { target });
        }
        let result = self.resolve_repository_target_inner(
            package, root_owner, root_path, segments, visibility, mode,
        );
        self.active_targets.remove(&key);
        match &result {
            Ok(origin) => {
                self.resolved_targets.insert(key, origin.clone());
            }
            Err(ArchitectureAuditError::UnresolvedRepositoryTarget { .. }) => {
                self.unresolved_targets.insert(key);
            }
            Err(_) => {}
        }
        result
    }

    fn resolve_repository_target_inner(
        &mut self,
        package: &str,
        root_owner: &str,
        root_path: &Path,
        segments: &[String],
        visibility: ImportVisibility,
        mode: RustDependencyMode,
    ) -> Result<CanonicalOrigin, ArchitectureAuditError> {
        let (module_length, mut declaration_file) = (0..=segments.len())
            .rev()
            .find_map(|module_length| {
                resolve_module_file(root_path, &segments[..module_length])
                    .map(|file| (module_length, file))
            })
            .ok_or_else(|| ArchitectureAuditError::UnresolvedRepositoryTarget {
                target: format!("{root_owner}::{}", segments.join("::")),
            })?;
        let module_segments = &segments[..module_length];
        let symbol_segments = &segments[module_length..];
        let is_wildcard_target = matches!(symbol_segments, [symbol] if symbol == "*");
        if !is_wildcard_target && let Some(first_symbol) = symbol_segments.first() {
            if let Some(origin) = self.resolve_import_alias(
                package,
                root_owner,
                root_path,
                &declaration_file,
                module_segments,
                first_symbol,
                &symbol_segments[1..],
                visibility,
                mode,
            )? {
                return Ok(origin);
            }
            declaration_file = self
                .find_symbol_declaration_file(&declaration_file, first_symbol)?
                .ok_or_else(|| ArchitectureAuditError::UnresolvedRepositoryTarget {
                    target: format!("{root_owner}::{}", segments.join("::")),
                })?;
        }
        let declaration_file = std::fs::canonicalize(&declaration_file).map_err(|source| {
            ArchitectureAuditError::Io {
                path: declaration_file.clone(),
                source,
            }
        })?;
        let repository_root =
            std::fs::canonicalize(&self.workspace.repository_root).map_err(|source| {
                ArchitectureAuditError::Io {
                    path: self.workspace.repository_root.clone(),
                    source,
                }
            })?;
        let relative_file = declaration_file
            .strip_prefix(repository_root)
            .map_err(|_| ArchitectureAuditError::UnresolvedRepositoryTarget {
                target: declaration_file.to_string_lossy().into_owned(),
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let fully_qualified_target = if segments.is_empty() {
            root_owner.to_owned()
        } else {
            format!("{root_owner}::{}", segments.join("::"))
        };
        let symbol = if is_wildcard_target {
            "*".to_owned()
        } else if symbol_segments.is_empty() {
            module_segments
                .last()
                .cloned()
                .unwrap_or_else(|| root_owner.to_owned())
        } else {
            symbol_segments.join("::")
        };
        Ok(CanonicalOrigin::Repository {
            package_name: package.to_owned(),
            repository_relative_declaration_file: relative_file,
            fully_qualified_target,
            symbol,
        })
    }
}

fn is_primitive_type(segment: &str) -> bool {
    matches!(
        segment,
        "bool"
            | "char"
            | "str"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
    )
}

fn root_for_dependency<'a>(
    workspace: &'a RustWorkspaceModel,
    dependency: &RawDependency,
) -> Result<&'a ProductionRoot, ArchitectureAuditError> {
    let owner = dependency
        .fully_qualified_owner
        .split("::")
        .next()
        .unwrap_or_default();
    workspace
        .roots
        .iter()
        .find(|root| {
            root.package == dependency.owning_package
                && root.target.replace('-', "_") == owner
                && root.kind != ProductionRootKind::Example
        })
        .or_else(|| {
            workspace
                .roots
                .iter()
                .find(|root| root.package == dependency.owning_package)
        })
        .ok_or_else(|| ArchitectureAuditError::UnknownDependencyTarget {
            owning_package: dependency.owning_package.clone(),
            target: dependency.fully_qualified_owner.clone(),
        })
}

fn split_target(target: &str) -> Result<Vec<String>, ArchitectureAuditError> {
    if target.is_empty() || target.contains('\\') || target.contains('/') {
        return Err(ArchitectureAuditError::InvalidDependencyTarget {
            target: target.to_owned(),
        });
    }
    let segments = target.split("::").map(str::to_owned).collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ArchitectureAuditError::InvalidDependencyTarget {
            target: target.to_owned(),
        });
    }
    Ok(segments)
}

fn nonempty_subpath(segments: &[String]) -> Option<String> {
    (!segments.is_empty()).then(|| segments.join("::"))
}

fn resolve_module_file(root: &Path, modules: &[String]) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for module in modules {
        if !module_is_declared(&current, module) {
            return None;
        }
        if let Some(explicit) = explicit_module_file(&current, module) {
            current = explicit;
            continue;
        }
        let parent = current.parent()?;
        let child_dir = if current.file_name().is_some_and(|name| name == "mod.rs") {
            parent.to_path_buf()
        } else if matches!(
            current.file_name().and_then(|name| name.to_str()),
            Some("lib.rs" | "main.rs" | "build.rs")
        ) {
            parent.to_path_buf()
        } else {
            parent.join(current.file_stem()?)
        };
        let flat = child_dir.join(format!("{module}.rs"));
        let directory = child_dir.join(module).join("mod.rs");
        current = match (flat.is_file(), directory.is_file()) {
            (true, false) => flat,
            (false, true) => directory,
            _ => return None,
        };
    }
    Some(current)
}

fn module_is_declared(current: &Path, module: &str) -> bool {
    let Ok(source) = std::fs::read_to_string(current) else {
        return false;
    };
    let Ok(syntax) = syn::parse_file(&source) else {
        return false;
    };
    syntax.items.into_iter().any(|item| {
        matches!(
            item,
            Item::Mod(item_mod)
                if !is_test_only(&item_mod.attrs)
                    && normalized_ident(&item_mod.ident) == module
        )
    })
}

fn explicit_module_file(current: &Path, module: &str) -> Option<PathBuf> {
    let source = std::fs::read_to_string(current).ok()?;
    let syntax = syn::parse_file(&source).ok()?;
    let item = syntax.items.into_iter().find_map(|item| match item {
        Item::Mod(item_mod) if normalized_ident(&item_mod.ident) == module => Some(item_mod),
        _ => None,
    })?;
    let attribute = item
        .attrs
        .into_iter()
        .find(|attribute| attribute.path().is_ident("path"))?;
    let Meta::NameValue(value) = attribute.meta else {
        return None;
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(path),
        ..
    }) = value.value
    else {
        return None;
    };
    let candidate = current.parent()?.join(path.value());
    candidate.is_file().then_some(candidate)
}

fn item_declares_symbol(item: &Item, symbol: &str) -> bool {
    match item {
        Item::Const(item) => normalized_ident(&item.ident) == symbol,
        Item::Enum(item) => normalized_ident(&item.ident) == symbol,
        Item::Fn(item) => normalized_ident(&item.sig.ident) == symbol,
        Item::Macro(item) => {
            item.ident
                .as_ref()
                .is_some_and(|ident| normalized_ident(ident) == symbol)
                || project_macro_declares_symbol(item, symbol)
        }
        Item::Mod(item) => normalized_ident(&item.ident) == symbol,
        Item::Static(item) => normalized_ident(&item.ident) == symbol,
        Item::Struct(item) => normalized_ident(&item.ident) == symbol,
        Item::Trait(item) => normalized_ident(&item.ident) == symbol,
        Item::TraitAlias(item) => normalized_ident(&item.ident) == symbol,
        Item::Type(item) => normalized_ident(&item.ident) == symbol,
        Item::Union(item) => normalized_ident(&item.ident) == symbol,
        Item::ExternCrate(_)
        | Item::ForeignMod(_)
        | Item::Impl(_)
        | Item::Use(_)
        | Item::Verbatim(_)
        | _ => false,
    }
}

fn project_macro_declares_symbol(item: &syn::ItemMacro, symbol: &str) -> bool {
    let Some(macro_name) = item.mac.path.get_ident().map(normalized_ident) else {
        return false;
    };
    if matches!(
        macro_name.as_str(),
        "fingerprint"
            | "index_type"
            | "opaque_id"
            | "opaque_resource_type"
            | "parameter_id"
            | "plan_id"
            | "runtime_id"
            | "string_identity"
            | "string_newtype"
            | "string_token"
            | "uuid_id"
    ) {
        return syn::parse2::<syn::Ident>(item.mac.tokens.clone())
            .is_ok_and(|ident| normalized_ident(&ident) == symbol);
    }
    if macro_name == "semantic_id" {
        return item
            .mac
            .tokens
            .clone()
            .into_iter()
            .next()
            .is_some_and(|token| {
                matches!(token, proc_macro2::TokenTree::Ident(ident) if normalized_ident(&ident) == symbol)
            });
    }
    matches!(
        (macro_name.as_str(), symbol),
        ("define_compiler_diagnostics", "CompilerDiagnostic")
            | (
                "define_compiler_diagnostics",
                "COMPILER_DIAGNOSTIC_DEFINITIONS"
            )
            | ("define_execution_demand", "ExecutionDemand")
            | ("define_execution_demand_dto", "ExecutionDemandDto")
            | ("define_run_event_kind", "RunEventKind")
    )
}

fn canonicalize_repository_source(
    repository_root: &Path,
    file: &Path,
) -> Result<PathBuf, ArchitectureAuditError> {
    let canonical = std::fs::canonicalize(file).map_err(|source| ArchitectureAuditError::Io {
        path: file.to_path_buf(),
        source,
    })?;
    if !canonical.starts_with(repository_root) {
        return Err(ArchitectureAuditError::SourceEscapesRepository {
            path: canonical,
            repository_root: repository_root.to_path_buf(),
        });
    }
    Ok(canonical)
}

#[derive(Clone)]
struct ImportAliasCandidates {
    paths: Vec<Vec<String>>,
    speculative: bool,
}

fn collect_use_targets(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    paths: &mut Vec<(Vec<String>, String)>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(normalized_ident(&path.ident));
            collect_use_targets(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            let ident = normalized_ident(&name.ident);
            let visible = if ident == "self" && !path.is_empty() {
                path.last().cloned().unwrap_or(ident)
            } else {
                path.push(ident.clone());
                ident
            };
            paths.push((path, visible));
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            let ident = normalized_ident(&rename.ident);
            if ident != "self" || path.is_empty() {
                path.push(ident);
            }
            paths.push((path, normalized_ident(&rename.rename)));
        }
        UseTree::Glob(_) => {
            let mut path = prefix.clone();
            path.push("*".to_owned());
            paths.push((path, "*".to_owned()));
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_use_targets(tree, prefix, paths);
            }
        }
    }
}

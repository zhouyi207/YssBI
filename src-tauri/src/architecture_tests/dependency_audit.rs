mod resolver;
mod visitor;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{Expr, ExprLit, Item, ItemMod, Lit, Meta};

use self::visitor::{ForbiddenDependencyVisitor, item_attributes};
use crate::architecture_tests::model::{
    ArchitectureAuditError, CanonicalDependency, ProductionRoot, ProductionRootKind, RawDependency,
    RustDependencyKind, RustDependencyMode, RustModule, RustWorkspaceModel,
};
use crate::test_support::source_audit::{
    has_production_cfg_attr_path, is_test_only, normalized_ident,
};

pub(super) struct DependencyResolutionFailure {
    dependency: RawDependency,
    error: ArchitectureAuditError,
}

impl std::fmt::Debug for DependencyResolutionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DependencyResolutionFailure")
            .field("dependency", &self.dependency)
            .field("error", &self.error)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DependencyViolation {
    file: String,
    module: String,
    reference: String,
}

#[derive(Debug, Clone)]
struct ModuleSource {
    file: PathBuf,
    child_module_dir: PathBuf,
    path_attr_dir: PathBuf,
    module_path: Vec<String>,
}

#[derive(Default)]
struct AuditState {
    visited: BTreeSet<(PathBuf, Vec<String>)>,
    violations: BTreeSet<DependencyViolation>,
}

fn audit_production_dependency(
    source_root: &Path,
    root_module: &str,
    forbidden_module: &str,
) -> Result<Vec<DependencyViolation>, String> {
    let source_root = canonicalize(source_root)?;
    let root = resolve_root_module(&source_root, root_module)?;
    let mut state = AuditState::default();
    audit_module_file(&source_root, root, forbidden_module, &mut state)?;
    Ok(state.violations.into_iter().collect())
}

pub(super) fn collect_production_dependencies(
    repository_root: &Path,
    roots: &[ProductionRoot],
) -> Result<Vec<RawDependency>, ArchitectureAuditError> {
    collect_production_graph(repository_root, roots).map(|(_, dependencies)| dependencies)
}

pub(super) fn resolve_canonical_dependencies_detailed(
    workspace: &RustWorkspaceModel,
    raw: &[RawDependency],
) -> Result<Vec<CanonicalDependency>, DependencyResolutionFailure> {
    resolver::resolve_canonical_dependencies_detailed(workspace, raw)
}

pub(super) fn collect_production_modules(
    repository_root: &Path,
    roots: &[ProductionRoot],
) -> Result<Vec<RustModule>, ArchitectureAuditError> {
    collect_production_graph(repository_root, roots).map(|(modules, _)| modules)
}

fn collect_production_graph(
    repository_root: &Path,
    roots: &[ProductionRoot],
) -> Result<(Vec<RustModule>, Vec<RawDependency>), ArchitectureAuditError> {
    let repository_root =
        std::fs::canonicalize(repository_root).map_err(|source| ArchitectureAuditError::Io {
            path: repository_root.to_path_buf(),
            source,
        })?;
    let mut modules = Vec::new();
    let mut dependencies = Vec::new();
    for root in roots {
        let source_path = canonicalize_under_audit_root(&repository_root, &root.source_path)?;
        let module = ModuleSource {
            child_module_dir: source_path
                .parent()
                .expect("Cargo root source must have a parent")
                .to_path_buf(),
            path_attr_dir: path_attr_dir_for_file(&source_path),
            file: source_path.clone(),
            module_path: root_module_path(root, &source_path),
        };
        let mode = if root.kind == ProductionRootKind::BuildScript {
            RustDependencyMode::Build
        } else {
            RustDependencyMode::Runtime
        };
        let mut visited = BTreeSet::new();
        collect_module_file(
            &repository_root,
            root,
            module,
            mode,
            &mut visited,
            &mut modules,
            &mut dependencies,
        )?;
    }
    modules.sort();
    modules.dedup();
    dependencies.sort();
    Ok((modules, dependencies))
}

fn collect_module_file(
    repository_root: &Path,
    root: &ProductionRoot,
    module: ModuleSource,
    mode: RustDependencyMode,
    visited: &mut BTreeSet<(PathBuf, Vec<String>)>,
    modules: &mut Vec<RustModule>,
    dependencies: &mut Vec<RawDependency>,
) -> Result<(), ArchitectureAuditError> {
    if !visited.insert((module.file.clone(), module.module_path.clone())) {
        return Ok(());
    }
    let source =
        std::fs::read_to_string(&module.file).map_err(|source| ArchitectureAuditError::Io {
            path: module.file.clone(),
            source,
        })?;
    let syntax =
        syn::parse_file(&source).map_err(|source| ArchitectureAuditError::SourceParse {
            path: module.file.clone(),
            source,
        })?;
    if is_test_only(&syntax.attrs) {
        return Ok(());
    }
    modules.push(RustModule {
        root_package_id: root.package_id.clone(),
        root_target: root.target.clone(),
        root_kind: root.kind,
        repository_relative_source_file: relative_source_path_audit(repository_root, &module.file)?,
        fully_qualified_owner: module_owner(root, &module),
    });
    collect_items(
        repository_root,
        root,
        &source,
        &syntax.items,
        &module,
        mode,
        visited,
        modules,
        dependencies,
    )
}

fn collect_items(
    repository_root: &Path,
    root: &ProductionRoot,
    source: &str,
    items: &[Item],
    module: &ModuleSource,
    mode: RustDependencyMode,
    visited: &mut BTreeSet<(PathBuf, Vec<String>)>,
    modules: &mut Vec<RustModule>,
    dependencies: &mut Vec<RawDependency>,
) -> Result<(), ArchitectureAuditError> {
    for item in items {
        if is_test_only(item_attributes(item)) {
            continue;
        }
        if let Item::Mod(item_mod) = item {
            collect_path_attribute(
                repository_root,
                root,
                source,
                item_mod,
                module,
                mode,
                dependencies,
            )?;
            if let Some((_, inline_items)) = &item_mod.content {
                let name = normalized_ident(&item_mod.ident);
                let path_base = explicit_module_path(module, item_mod)
                    .map_err(|message| ArchitectureAuditError::InvalidMetadata { message })?
                    .map(|path| module.path_attr_dir.join(path))
                    .unwrap_or_else(|| module.child_module_dir.join(&name));
                let mut inline = module.clone();
                inline.child_module_dir = path_base.clone();
                inline.path_attr_dir = path_base;
                inline.module_path.push(name);
                collect_items(
                    repository_root,
                    root,
                    source,
                    inline_items,
                    &inline,
                    mode,
                    visited,
                    modules,
                    dependencies,
                )?;
            } else {
                let child = resolve_external_module(repository_root, module, item_mod)
                    .map_err(|message| ArchitectureAuditError::InvalidMetadata { message })?;
                collect_module_file(
                    repository_root,
                    root,
                    child,
                    mode,
                    visited,
                    modules,
                    dependencies,
                )?;
            }
            continue;
        }

        let file = relative_source_path_audit(repository_root, &module.file)?;
        let owner = module_owner(root, module);
        let dependency_start = dependencies.len();
        let mut visitor = visitor::RawDependencyVisitor::new(
            &root.package,
            &file,
            &owner,
            mode,
            source,
            dependencies,
        );
        visitor.visit_item(item);
        let unresolved_include = visitor.unresolved_include();
        let code_includes = visitor.code_includes();
        drop(visitor);
        if let Some(target) = unresolved_include {
            return Err(ArchitectureAuditError::UnresolvedInclude {
                source_file: module.file.clone(),
                target,
            });
        }
        for dependency in &dependencies[dependency_start..] {
            if dependency.kind == RustDependencyKind::Include {
                let include_path = module
                    .file
                    .parent()
                    .unwrap_or(repository_root)
                    .join(&dependency.written_target);
                canonicalize_under_audit_root(repository_root, &include_path)?;
            }
        }
        for target in code_includes {
            let include_path = module.file.parent().unwrap_or(repository_root).join(target);
            let file = canonicalize_under_audit_root(repository_root, &include_path)?;
            let included_module = ModuleSource {
                child_module_dir: child_module_dir_for_file(&file),
                path_attr_dir: path_attr_dir_for_file(&file),
                file,
                module_path: module.module_path.clone(),
            };
            collect_module_file(
                repository_root,
                root,
                included_module,
                mode,
                visited,
                modules,
                dependencies,
            )?;
        }
    }
    Ok(())
}

fn collect_path_attribute(
    repository_root: &Path,
    root: &ProductionRoot,
    source: &str,
    item_mod: &ItemMod,
    module: &ModuleSource,
    mode: RustDependencyMode,
    dependencies: &mut Vec<RawDependency>,
) -> Result<(), ArchitectureAuditError> {
    let Some(attribute) = item_mod
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
    else {
        return Ok(());
    };
    let Some(target) =
        attribute
            .meta
            .require_name_value()
            .ok()
            .and_then(|value| match &value.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(path),
                    ..
                }) => Some(path.value()),
                _ => None,
            })
    else {
        return Ok(());
    };
    let file = relative_source_path_audit(repository_root, &module.file)?;
    let owner = module_owner(root, module);
    let (_, line, column) = visitor::source_location(source, &target, 0);
    dependencies.push(RawDependency {
        owning_package: root.package.clone(),
        repository_relative_source_file: file,
        fully_qualified_owner: owner,
        kind: RustDependencyKind::Attribute,
        mode,
        written_target: target,
        line,
        column,
    });
    Ok(())
}

fn root_module_path(root: &ProductionRoot, source_path: &Path) -> Vec<String> {
    let owner = root_owner(root);
    let Some(file_name) = source_path.file_name().and_then(|name| name.to_str()) else {
        return vec![owner];
    };
    if file_name == "mod.rs" {
        source_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .map(|name| vec![name.to_owned()])
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn root_owner(root: &ProductionRoot) -> String {
    root.target.replace('-', "_")
}

fn module_owner(root: &ProductionRoot, module: &ModuleSource) -> String {
    let root_owner = root_owner(root);
    if module.module_path.is_empty() {
        root_owner
    } else {
        format!("{root_owner}::{}", module.module_path.join("::"))
    }
}

fn canonicalize_under_audit_root(
    repository_root: &Path,
    path: &Path,
) -> Result<PathBuf, ArchitectureAuditError> {
    let canonical = std::fs::canonicalize(path).map_err(|source| ArchitectureAuditError::Io {
        path: path.to_path_buf(),
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

fn relative_source_path_audit(
    repository_root: &Path,
    file: &Path,
) -> Result<String, ArchitectureAuditError> {
    let relative = file.strip_prefix(repository_root).map_err(|_| {
        ArchitectureAuditError::SourceEscapesRepository {
            path: file.to_path_buf(),
            repository_root: repository_root.to_path_buf(),
        }
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn resolve_root_module(source_root: &Path, module: &str) -> Result<ModuleSource, String> {
    let flat = source_root.join(format!("{module}.rs"));
    let directory = source_root.join(module).join("mod.rs");
    let file = match (flat.is_file(), directory.is_file()) {
        (true, false) => flat,
        (false, true) => directory,
        (true, true) => {
            return Err(format!(
                "root module '{module}' is ambiguous: '{}' and '{}' both exist",
                flat.display(),
                directory.display()
            ));
        }
        (false, false) => return Err(format!("root module '{module}' was not found")),
    };
    let file = canonicalize_under(source_root, &file)?;
    let child_module_dir = if matches!(
        file.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs")
    ) {
        source_root.to_path_buf()
    } else {
        child_module_dir_for_file(&file)
    };
    Ok(ModuleSource {
        child_module_dir,
        path_attr_dir: path_attr_dir_for_file(&file),
        file,
        module_path: vec![module.to_owned()],
    })
}

fn audit_module_file(
    source_root: &Path,
    module: ModuleSource,
    forbidden_module: &str,
    state: &mut AuditState,
) -> Result<(), String> {
    if !state
        .visited
        .insert((module.file.clone(), module.module_path.clone()))
    {
        return Ok(());
    }
    let source = std::fs::read_to_string(&module.file)
        .map_err(|error| format!("failed to read '{}': {error}", module.file.display()))?;
    let syntax = syn::parse_file(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", module.file.display()))?;
    if is_test_only(&syntax.attrs) {
        return Ok(());
    }
    audit_items(source_root, &syntax.items, &module, forbidden_module, state)
}

fn audit_items(
    source_root: &Path,
    items: &[Item],
    module: &ModuleSource,
    forbidden_module: &str,
    state: &mut AuditState,
) -> Result<(), String> {
    for item in items {
        if is_test_only(item_attributes(item)) {
            continue;
        }
        if let Item::Mod(item_mod) = item {
            if let Some((_, inline_items)) = &item_mod.content {
                let name = normalized_ident(&item_mod.ident);
                let path_base = explicit_module_path(module, item_mod)?
                    .map(|path| module.path_attr_dir.join(path))
                    .unwrap_or_else(|| module.child_module_dir.join(&name));
                let mut inline = module.clone();
                inline.child_module_dir = path_base.clone();
                inline.path_attr_dir = path_base;
                inline.module_path.push(name);
                audit_items(source_root, inline_items, &inline, forbidden_module, state)?;
            } else {
                let child = resolve_external_module(source_root, module, item_mod)?;
                audit_module_file(source_root, child, forbidden_module, state)?;
            }
            continue;
        }

        let file = relative_source_path(source_root, &module.file)?;
        let module_name = format!("crate::{}", module.module_path.join("::"));
        let mut visitor = ForbiddenDependencyVisitor {
            forbidden_module,
            file: &file,
            module: &module_name,
            violations: &mut state.violations,
        };
        visitor.visit_item(item);
    }
    Ok(())
}

fn resolve_external_module(
    source_root: &Path,
    parent: &ModuleSource,
    item: &ItemMod,
) -> Result<ModuleSource, String> {
    let name = normalized_ident(&item.ident);
    let file = if let Some(explicit) = explicit_module_path(parent, item)? {
        let path = parent.path_attr_dir.join(explicit);
        if !path.is_file() {
            return Err(format!(
                "module '{name}' declared by '{}' was not found at '{}'",
                parent.file.display(),
                path.display()
            ));
        }
        path
    } else {
        let flat = parent.child_module_dir.join(format!("{name}.rs"));
        let directory = parent.child_module_dir.join(&name).join("mod.rs");
        match (flat.is_file(), directory.is_file()) {
            (true, false) => flat,
            (false, true) => directory,
            (true, true) => {
                return Err(format!(
                    "module '{name}' declared by '{}' is ambiguous: '{}' and '{}' both exist",
                    parent.file.display(),
                    flat.display(),
                    directory.display()
                ));
            }
            (false, false) => {
                return Err(format!(
                    "module '{name}' declared by '{}' was not found; checked '{}' and '{}'",
                    parent.file.display(),
                    flat.display(),
                    directory.display()
                ));
            }
        }
    };
    let file = canonicalize_under(source_root, &file)?;
    let mut module_path = parent.module_path.clone();
    module_path.push(name);
    Ok(ModuleSource {
        child_module_dir: child_module_dir_for_file(&file),
        path_attr_dir: path_attr_dir_for_file(&file),
        file,
        module_path,
    })
}

fn explicit_module_path(parent: &ModuleSource, item: &ItemMod) -> Result<Option<PathBuf>, String> {
    let module_name = normalized_ident(&item.ident);
    if has_production_cfg_attr_path(&item.attrs) {
        let mut logical_path = parent.module_path.clone();
        logical_path.push(module_name);
        return Err(format!(
            "module 'crate::{}' declared by '{}' has a production-reachable cfg_attr that can emit path",
            logical_path.join("::"),
            parent.file.to_string_lossy().replace('\\', "/")
        ));
    }
    let path_attributes = item
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("path"))
        .collect::<Vec<_>>();
    if path_attributes.is_empty() {
        return Ok(None);
    }
    if path_attributes.len() != 1 {
        return Err(format!(
            "module '{}' in '{}' has multiple #[path] attributes",
            normalized_ident(&item.ident),
            parent.file.display()
        ));
    }
    let Meta::NameValue(value) = &path_attributes[0].meta else {
        return Err(format!(
            "module '{}' in '{}' has an invalid #[path] attribute",
            normalized_ident(&item.ident),
            parent.file.display()
        ));
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(path),
        ..
    }) = &value.value
    else {
        return Err(format!(
            "module '{}' in '{}' has a non-string #[path] attribute",
            normalized_ident(&item.ident),
            parent.file.display()
        ));
    };
    Ok(Some(PathBuf::from(path.value())))
}

fn child_module_dir_for_file(file: &Path) -> PathBuf {
    let parent = file.parent().expect("Rust source file must have a parent");
    if file.file_name().is_some_and(|name| name == "mod.rs") {
        parent.to_path_buf()
    } else {
        parent.join(file.file_stem().expect("Rust source file must have a stem"))
    }
}

fn path_attr_dir_for_file(file: &Path) -> PathBuf {
    file.parent()
        .expect("Rust source file must have a parent")
        .to_path_buf()
}

fn canonicalize(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))
}

fn canonicalize_under(source_root: &Path, path: &Path) -> Result<PathBuf, String> {
    let canonical = canonicalize(path)?;
    if !canonical.starts_with(source_root) {
        return Err(format!(
            "module source '{}' escapes source root '{}'",
            canonical.display(),
            source_root.display()
        ));
    }
    Ok(canonical)
}

fn relative_source_path(source_root: &Path, file: &Path) -> Result<String, String> {
    let relative = file.strip_prefix(source_root).map_err(|_| {
        format!(
            "module source '{}' is outside '{}'",
            file.display(),
            source_root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

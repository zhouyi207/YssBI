mod visitor;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{Expr, ExprLit, Item, ItemMod, Lit, Meta};

use self::visitor::{ForbiddenDependencyVisitor, item_attributes};
use crate::test_support::source_audit::is_test_only;

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
    Ok(ModuleSource {
        child_module_dir: child_module_dir_for_file(&file),
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
                let name = item_mod.ident.to_string();
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
    let name = item.ident.to_string();
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
            item.ident,
            parent.file.display()
        ));
    }
    let Meta::NameValue(value) = &path_attributes[0].meta else {
        return Err(format!(
            "module '{}' in '{}' has an invalid #[path] attribute",
            item.ident,
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
            item.ident,
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

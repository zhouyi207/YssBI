use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::model::{
    ArchitectureAuditError, CargoDependencyAuthority, CargoDependencyDeclaration,
    CargoDependencyScope, ProductionRoot, ProductionRootKind, RustWorkspaceModel,
    WorkspaceMemberCrateAlias,
};

pub(super) fn discover_rust_workspace_model(
    workspace_manifest: &Path,
) -> Result<RustWorkspaceModel, ArchitectureAuditError> {
    let manifest = canonicalize_path(workspace_manifest)?;
    let repository_root = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| ArchitectureAuditError::InvalidMetadata {
            message: format!(
                "workspace manifest '{}' has no repository parent",
                manifest.display()
            ),
        })?
        .to_path_buf();
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .map_err(|source| ArchitectureAuditError::Io {
            path: manifest.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(ArchitectureAuditError::MetadataProcess {
            status: output.status.code().map_or_else(
                || "terminated-by-signal".to_owned(),
                |code| code.to_string(),
            ),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let metadata = serde_json::from_slice::<Value>(&output.stdout)
        .map_err(|source| ArchitectureAuditError::MetadataJson { source })?;
    rust_workspace_model_from_metadata(&repository_root, metadata)
}

pub(super) fn rust_workspace_model_from_metadata(
    repository_root: &Path,
    metadata: Value,
) -> Result<RustWorkspaceModel, ArchitectureAuditError> {
    let repository_root = canonicalize_path(repository_root)?;
    let object = metadata
        .as_object()
        .ok_or_else(|| invalid("metadata root must be an object"))?;
    let workspace_members = string_array(object, "workspace_members")?;
    let packages = object
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("metadata packages must be an array"))?;

    let member_ids = workspace_members.iter().cloned().collect::<BTreeSet<_>>();
    let mut packages_by_id = BTreeMap::new();
    let mut member_ids_by_name = BTreeMap::<String, String>::new();
    for package in packages {
        let package_object = package
            .as_object()
            .ok_or_else(|| invalid("package entry must be an object"))?;
        let id = required_string(package_object, "id")?;
        if !member_ids.contains(&id) {
            continue;
        }
        let name = required_string(package_object, "name")?;
        if member_ids_by_name
            .insert(name.clone(), id.clone())
            .is_some()
        {
            return Err(ArchitectureAuditError::AmbiguousWorkspacePackage { package: name });
        }
        packages_by_id.insert(id, package_object);
    }
    for member_id in &workspace_members {
        if !packages_by_id.contains_key(member_id) {
            return Err(invalid(format!(
                "workspace member '{member_id}' has no package entry"
            )));
        }
    }

    let mut roots = Vec::new();
    let mut library_targets = BTreeMap::<String, (String, PathBuf)>::new();
    for member_id in &workspace_members {
        let package = packages_by_id.get(member_id).ok_or_else(|| {
            invalid(format!(
                "workspace member '{member_id}' has no package entry"
            ))
        })?;
        let package_name = required_string(package, "name")?;
        let targets = package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("package '{package_name}' targets must be an array")))?;
        for target in targets {
            let target = target
                .as_object()
                .ok_or_else(|| invalid("target entry must be an object"))?;
            let target_name = required_string(target, "name")?;
            let target_kinds = target
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(format!("target '{target_name}' kind must be an array")))?;
            let target_kind = production_root_kind(target_kinds, &package_name, &target_name)?;
            let Some(target_kind) = target_kind else {
                continue;
            };
            let source = required_string(target, "src_path")?;
            let source_path = canonicalize_source_path(&repository_root, Path::new(&source))?;
            roots.push(ProductionRoot {
                package_id: member_id.clone(),
                package: package_name.clone(),
                target: target_name.clone(),
                kind: target_kind,
                source_path: source_path.clone(),
            });
            if target_kind == ProductionRootKind::Library {
                if library_targets
                    .insert(member_id.clone(), (target_name.clone(), source_path))
                    .is_some()
                {
                    return Err(invalid(format!(
                        "workspace member '{package_name}' has multiple library targets"
                    )));
                }
            }
        }
    }

    let mut dependency_declarations = Vec::new();
    let mut workspace_member_crate_aliases = Vec::new();
    for member_id in &workspace_members {
        let package = packages_by_id.get(member_id).ok_or_else(|| {
            invalid(format!(
                "workspace member '{member_id}' has no package entry"
            ))
        })?;
        let owning_package = required_string(package, "name")?;
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(format!(
                    "package '{owning_package}' dependencies must be an array"
                ))
            })?;
        for dependency in dependencies {
            let dependency = dependency
                .as_object()
                .ok_or_else(|| invalid("dependency entry must be an object"))?;
            let dependency_name = required_string(dependency, "name")?;
            let package_name = dependency
                .get("package")
                .and_then(Value::as_str)
                .unwrap_or(&dependency_name)
                .to_owned();
            let declared_name = dependency
                .get("rename")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| dependency_name.replace('-', "_"));
            let scope = dependency_scope(dependency.get("kind"))?;
            let target_condition = dependency
                .get("target")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let authority = member_ids_by_name.get(&package_name).map_or(
                CargoDependencyAuthority::External,
                |member_package_id| CargoDependencyAuthority::WorkspaceMember {
                    member_package_id: member_package_id.clone(),
                },
            );
            dependency_declarations.push(CargoDependencyDeclaration {
                owning_package_id: member_id.clone(),
                owning_package: owning_package.clone(),
                declared_name: declared_name.clone(),
                package_name: package_name.clone(),
                authority: authority.clone(),
                scope,
                target_condition,
            });

            let CargoDependencyAuthority::WorkspaceMember { member_package_id } = authority else {
                continue;
            };
            let (library_crate_name, library_root) = library_targets
                .get(&member_package_id)
                .cloned()
                .ok_or_else(|| ArchitectureAuditError::MissingLibraryTarget {
                    package: package_name.clone(),
                })?;
            workspace_member_crate_aliases.push(WorkspaceMemberCrateAlias {
                owning_package_id: member_id.clone(),
                owning_package: owning_package.clone(),
                declared_name,
                member_package_id,
                member_package: package_name,
                root_owner: library_crate_name.clone(),
                library_crate_name,
                library_root,
            });
        }
    }

    roots.sort_by(|left, right| {
        (&left.package, &left.target, left.kind, &left.source_path).cmp(&(
            &right.package,
            &right.target,
            right.kind,
            &right.source_path,
        ))
    });
    dependency_declarations.sort();
    dependency_declarations.dedup();
    workspace_member_crate_aliases.sort();
    workspace_member_crate_aliases.dedup();

    Ok(RustWorkspaceModel {
        repository_root,
        roots,
        dependency_declarations,
        workspace_member_crate_aliases,
    })
}

fn production_root_kind(
    kinds: &[Value],
    package: &str,
    target: &str,
) -> Result<Option<ProductionRootKind>, ArchitectureAuditError> {
    let mut production = None;
    for kind in kinds.iter().filter_map(Value::as_str) {
        let Some(candidate) = (match kind {
            "lib" | "rlib" | "staticlib" | "cdylib" => Some(ProductionRootKind::Library),
            "bin" => Some(ProductionRootKind::Binary),
            "example" => Some(ProductionRootKind::Example),
            "custom-build" => Some(ProductionRootKind::BuildScript),
            "test" | "bench" => None,
            _ => {
                return Err(invalid(format!(
                    "target '{package}::{target}' has unknown Cargo target kind '{kind}'"
                )));
            }
        }) else {
            continue;
        };
        if production.is_some_and(|existing| existing != candidate) {
            return Err(invalid(format!(
                "target '{package}::{target}' has ambiguous production kinds"
            )));
        }
        production = Some(candidate);
    }
    let Some(first) = production else {
        return Ok(None);
    };
    Ok(Some(first))
}

fn dependency_scope(kind: Option<&Value>) -> Result<CargoDependencyScope, ArchitectureAuditError> {
    match kind.and_then(Value::as_str) {
        None | Some("normal") => Ok(CargoDependencyScope::Runtime),
        Some("build") => Ok(CargoDependencyScope::Build),
        Some("dev") => Ok(CargoDependencyScope::Development),
        Some(value) => Err(invalid(format!("unknown Cargo dependency kind '{value}'"))),
    }
}

fn canonicalize_path(path: &Path) -> Result<PathBuf, ArchitectureAuditError> {
    std::fs::canonicalize(path).map_err(|source| ArchitectureAuditError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn canonicalize_source_path(
    repository_root: &Path,
    source: &Path,
) -> Result<PathBuf, ArchitectureAuditError> {
    let source_path = canonicalize_path(source)?;
    if !source_path.starts_with(repository_root) {
        return Err(ArchitectureAuditError::SourceEscapesRepository {
            path: source_path,
            repository_root: repository_root.to_path_buf(),
        });
    }
    Ok(source_path)
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, ArchitectureAuditError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("metadata field '{key}' must be a non-empty string")))
}

fn string_array(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, ArchitectureAuditError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("metadata field '{key}' must be an array")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("metadata field '{key}' must contain strings")))
        })
        .collect()
}

fn invalid(message: impl Into<String>) -> ArchitectureAuditError {
    ArchitectureAuditError::InvalidMetadata {
        message: message.into(),
    }
}

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::audit_production_dependency;
use super::collect_production_dependencies;
use super::visitor::source_location;
use crate::architecture_tests::model::{
    ArchitectureAuditError, CanonicalOrigin, CargoDependencyAuthority, CargoDependencyDeclaration,
    CargoDependencyScope, ExternalDependencyOrigin, ProductionRoot, ProductionRootKind,
    RawDependency, RustDependencyKind, RustDependencyMode, RustWorkspaceModel,
    WorkspaceMemberCrateAlias,
};

const FIXTURE_PREFIX: &str = "architecture-dependency-audit-";

struct FixtureTree {
    root: PathBuf,
    source_root: PathBuf,
}

impl FixtureTree {
    fn new(label: &str) -> Self {
        let target = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri must have a repository parent")
            .join("target");
        let root = target.join(format!("{FIXTURE_PREFIX}{label}-{}", uuid::Uuid::new_v4()));
        let source_root = root.join("src");
        std::fs::create_dir_all(source_root.join("project"))
            .expect("fixture project directory must be created");
        Self { root, source_root }
    }

    fn write(&self, relative: &str, source: &str) {
        let path = self.source_root.join(relative);
        assert!(path.starts_with(&self.source_root));
        std::fs::create_dir_all(path.parent().expect("fixture file must have a parent"))
            .expect("fixture parent directory must be created");
        std::fs::write(path, source).expect("fixture source must be written");
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        let safe_name = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(FIXTURE_PREFIX));
        if safe_name {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn project_dependency_audit_respects_production_module_reachability() {
    let fixture = FixtureTree::new("reachability");
    fixture.write(
        "project/mod.rs",
        r#"
mod production;
mod r#raw_module;
mod included_wrapper;

#[path = "redirected.rs"]
mod redirected;

#[cfg(test)]
mod tests;

#[cfg(all(test, windows))]
mod windows_tests;

#[cfg(any(test, feature = "fixture"))]
mod mixed;

mod inner_tests;

mod macro_only;

mod nested {
    #[path = "child.rs"]
    mod child;
}

#[cfg(test)]
mod inline_tests {
    use crate::application::ignored_inline;
}
"#,
    );
    fixture.write(
        "project/production.rs",
        r#"
use crate::application::database;
use crate::{application as app, database as data};

type Handler = crate::application::Handler;

#[path = "production_child.rs"]
mod production_child;

fn invoke() {
    application::run();
    matches!((), crate::application::Type);
}
"#,
    );
    fixture.write(
        "project/redirected.rs",
        "use crate::application::redirected;",
    );
    fixture.write("project/tests.rs", "use crate::application::ignored;");
    fixture.write(
        "project/windows_tests.rs",
        "use crate::application::ignored_all_test;",
    );
    fixture.write("project/mixed.rs", "use crate::application::mixed;");
    fixture.write(
        "project/inner_tests.rs",
        "#![cfg(test)]\nuse crate::application::ignored_inner_test;",
    );
    fixture.write(
        "project/macro_only.rs",
        "fn emit() { trace!(application); }",
    );
    fixture.write("project/nested/child.rs", "use crate::application::nested;");
    fixture.write(
        "project/production_child.rs",
        "use crate::application::production_child;",
    );
    fixture.write(
        "project/raw_module.rs",
        r#"
use crate::r#application::raw_dependency;

type RawHandler = crate::r#application::RawHandler;

fn raw_macro_path() {
    matches!((), crate::r#application::RawVariant);
}
"#,
    );
    fixture.write(
        "project/included_wrapper.rs",
        r#"
include!("included_body.rs");
const INCLUDED_TEXT: &str = include_str!("included_body.rs");
const INCLUDED_BYTES: &[u8] = include_bytes!("included_body.rs");
"#,
    );
    fixture.write(
        "project/included_body.rs",
        "use crate::included::Target;\nfn included_body() {}\n",
    );

    let violations = audit_production_dependency(&fixture.source_root, "project", "application")
        .expect("fixture dependency audit must complete");
    let files = violations
        .iter()
        .map(|violation| violation.file.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        files,
        BTreeSet::from([
            "project/included_wrapper.rs",
            "project/mixed.rs",
            "project/nested/child.rs",
            "project/production.rs",
            "project/production_child.rs",
            "project/raw_module.rs",
            "project/redirected.rs",
        ])
    );
    assert!(!files.contains("project/inner_tests.rs"));
    assert!(!files.contains("project/macro_only.rs"));

    let production_references = violations
        .iter()
        .filter(|violation| violation.file == "project/production.rs")
        .map(|violation| violation.reference.as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "crate::application",
        "crate::application::Handler",
        "crate::application::database",
        "application::run",
        "macro-token::application",
    ] {
        assert!(
            production_references.contains(expected),
            "missing dependency reference {expected}: {production_references:?}"
        );
    }

    let raw_violations = violations
        .iter()
        .filter(|violation| violation.file == "project/raw_module.rs")
        .collect::<Vec<_>>();
    assert!(
        raw_violations
            .iter()
            .all(|violation| violation.module == "crate::project::raw_module")
    );
    assert_eq!(
        raw_violations
            .iter()
            .map(|violation| violation.reference.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "crate::application::RawHandler",
            "crate::application::raw_dependency",
            "macro-token::application",
        ])
    );

    let included_references = violations
        .iter()
        .filter(|violation| violation.file == "project/included_wrapper.rs")
        .map(|violation| violation.reference.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        included_references,
        BTreeSet::from(["macro-include!::<unexpanded>"])
    );

    let cfg_attr_fixture = FixtureTree::new("cfg-attr-path");
    cfg_attr_fixture.write(
        "project/mod.rs",
        r#"
#[cfg_attr(test, path = "test_only.rs")]
mod harmless;

#[cfg_attr(not(test), path = "production.rs")]
mod selected;
"#,
    );
    cfg_attr_fixture.write("project/harmless.rs", "fn harmless() {}\n");
    cfg_attr_fixture.write("project/selected.rs", "fn harmless() {}\n");

    let error =
        audit_production_dependency(&cfg_attr_fixture.source_root, "project", "application")
            .expect_err("production cfg_attr(path) must fail closed");
    assert!(
        error.contains("production-reachable cfg_attr")
            && error.contains("path")
            && error.contains("selected")
            && error.contains("project/mod.rs"),
        "unexpected cfg_attr(path) error: {error}"
    );
}

#[test]
fn production_project_modules_do_not_depend_on_application() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for forbidden_module in ["application", "commands"] {
        let violations = audit_production_dependency(&source_root, "project", forbidden_module)
            .unwrap_or_else(|error| {
                panic!("Project dependency audit for {forbidden_module} must complete: {error}")
            });
        assert!(
            violations.is_empty(),
            "production Project modules must not depend on {forbidden_module}:\n{}",
            violations
                .iter()
                .map(|violation| format!(
                    "{} [{}] -> {}",
                    violation.file, violation.module, violation.reference
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn production_dependency_collection_preserves_raw_fact_kinds_and_modes() {
    let fixture = FixtureTree::new("raw-facts");
    fixture.write(
        "project/mod.rs",
        r#"
#[path = "redirected.rs"]
mod redirected;

#[cfg(test)]
mod ignored_tests;

#[cfg(any(test, feature = "fixture"))]
mod mixed;

mod inline {
    use crate::inline::Thing;
}

#[doc(hidden)]
pub struct Hidden;

pub use crate::facade::GraphCompiler;
use crate::{graph::Graph, facade::GraphCompiler as Compiler};
use crate::graph::{self, Graph as ImportedGraph};

#[allow(clippy::too_many_arguments)]
fn run() {
    let _: crate::graph::Graph = crate::graph::make();
    let _ = Vec::<u8>::new();
    let params = 1;
    let _ = params;
    let _ = format!("{params}");
    tracing::trace!(crate::graph::trace);
}

include!("included_body.rs");
"#,
    );
    fixture.write("project/redirected.rs", "use crate::redirected::Target;\n");
    fixture.write("project/mixed.rs", "use crate::mixed::Target;\n");
    fixture.write("project/ignored_tests.rs", "use crate::ignored::Target;\n");
    fixture.write(
        "project/included_body.rs",
        "use crate::included::Target;\nfn included_body() {}\n",
    );
    fixture.write("build.rs", "fn main() { tauri_build::build(); }\n");

    let roots = vec![
        ProductionRoot {
            package_id: "fixture-package".to_owned(),
            package: "fixture".to_owned(),
            target: "fixture_lib".to_owned(),
            kind: ProductionRootKind::Library,
            source_path: fixture.source_root.join("project/mod.rs"),
        },
        ProductionRoot {
            package_id: "fixture-package".to_owned(),
            package: "fixture".to_owned(),
            target: "build-script-build".to_owned(),
            kind: ProductionRootKind::BuildScript,
            source_path: fixture.source_root.join("build.rs"),
        },
    ];

    let facts = collect_production_dependencies(&fixture.root, &roots)
        .expect("raw dependency collection must complete");
    assert!(facts.iter().all(|fact| fact.line > 0 && fact.column > 0));

    let contains = |kind: RustDependencyKind, mode: RustDependencyMode, target: &str| {
        facts
            .iter()
            .any(|fact| fact.kind == kind && fact.mode == mode && fact.written_target == target)
    };
    assert!(contains(
        RustDependencyKind::Use,
        RustDependencyMode::Runtime,
        "crate::graph::Graph"
    ));
    assert!(contains(
        RustDependencyKind::Use,
        RustDependencyMode::Runtime,
        "crate::graph"
    ));
    assert!(
        !facts
            .iter()
            .any(|fact| fact.written_target == "crate::graph::self")
    );
    assert!(contains(
        RustDependencyKind::ReExport,
        RustDependencyMode::Runtime,
        "crate::facade::GraphCompiler"
    ));
    assert!(contains(
        RustDependencyKind::Path,
        RustDependencyMode::Runtime,
        "crate::graph::Graph"
    ));
    assert!(!contains(
        RustDependencyKind::Path,
        RustDependencyMode::Runtime,
        "Vec::new"
    ));
    assert!(contains(
        RustDependencyKind::Macro,
        RustDependencyMode::Runtime,
        "tracing::trace"
    ));
    assert!(contains(
        RustDependencyKind::Include,
        RustDependencyMode::Runtime,
        "included_body.rs"
    ));
    assert!(contains(
        RustDependencyKind::Use,
        RustDependencyMode::Runtime,
        "crate::included::Target"
    ));
    assert!(!facts.iter().any(|fact| fact.written_target == "doc"));
    assert!(
        !facts
            .iter()
            .any(|fact| fact.written_target == "clippy::too_many_arguments")
    );
    assert!(!facts.iter().any(|fact| fact.written_target == "params"));
    assert!(!facts.iter().any(|fact| fact.written_target == "format"));
    assert!(contains(
        RustDependencyKind::Attribute,
        RustDependencyMode::Runtime,
        "redirected.rs"
    ));
    assert!(contains(
        RustDependencyKind::Path,
        RustDependencyMode::Build,
        "tauri_build::build"
    ));
    assert!(
        facts
            .iter()
            .any(|fact| fact.repository_relative_source_file == "src/build.rs")
    );
    assert!(facts.iter().any(|fact| {
        fact.mode == RustDependencyMode::Runtime
            && fact.fully_qualified_owner == "fixture_lib::project"
    }));
    assert!(facts.iter().any(|fact| {
        fact.mode == RustDependencyMode::Build && fact.fully_qualified_owner == "build_script_build"
    }));
    assert!(
        !facts
            .iter()
            .any(|fact| fact.written_target == "crate::ignored::Target")
    );
}

#[test]
fn production_dependency_collection_fails_closed_for_dynamic_include() {
    let fixture = FixtureTree::new("dynamic-include");
    fixture.write("project/mod.rs", "include!(concat!(\"body\", \".rs\"));\n");
    fixture.write("project/body.rs", "fn body() {}\n");
    let roots = vec![ProductionRoot {
        package_id: "fixture-package".to_owned(),
        package: "fixture".to_owned(),
        target: "fixture_lib".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: fixture.source_root.join("project/mod.rs"),
    }];

    let error = collect_production_dependencies(&fixture.root, &roots)
        .expect_err("non-literal include targets must fail closed");
    assert!(matches!(
        error,
        ArchitectureAuditError::UnresolvedInclude { .. }
    ));
}

#[test]
fn source_location_fallback_advances_on_a_utf8_boundary() {
    let source = "use crate; // 对\nuse std;\n";
    let (next_cursor, line, column) = source_location(source, "crate::Type", 0);

    assert_eq!((line, column), (1, 5));
    assert_eq!(next_cursor, "use crate".len());
    assert!(source.is_char_boundary(next_cursor));

    let (_, next_line, next_column) = source_location(source, "std", next_cursor);
    assert_eq!((next_line, next_column), (2, 5));
}

#[test]
fn canonical_dependency_resolution_prefers_workspace_members_and_preserves_external_facts() {
    let fixture = FixtureTree::new("canonical-resolution");
    fixture.write(
        "lib.rs",
        "pub mod facade;\npub mod glob_facade;\npub mod graph;\n",
    );
    fixture.write(
        "main.rs",
        "fn main() { fixture_lib::facade::GraphCompiler; }\n",
    );
    fixture.write(
        "facade.rs",
        "use ndarray::Array1;\nuse polars::prelude::*;\nuse crate::graph::compiler::GraphCompiler as PrivateCompiler;\nmod child;\npub mod local;\npub use local::Local;\npub use crate::graph::compiler::GraphCompiler;\ninclude!(\"facade/included.rs\");\nmacro_rules! string_identity { ($name:ident) => { pub struct $name; }; }\nmacro_rules! uuid_id { ($name:ident) => { pub struct $name; }; }\nmacro_rules! semantic_id { ($name:ident, $label:literal) => { pub struct $name; }; }\nmacro_rules! define_execution_demand { ($variant:ident) => { pub enum ExecutionDemand { $variant } }; }\nstring_identity!(Generated);\nuuid_id!(GeneratedId);\nsemantic_id!(GeneratedSemantic, \"semantic\");\ndefine_execution_demand!(Default);\n",
    );
    fixture.write("facade/child.rs", "use super::PrivateCompiler;\n");
    fixture.write(
        "facade/local.rs",
        "pub struct Local;\nimpl Local { pub fn new() -> Self { Self } }\n",
    );
    fixture.write("facade/included.rs", "pub struct Included;\n");
    fixture.write(
        "glob_facade.rs",
        "pub mod recursive;\npub mod target;\npub use recursive::*;\npub use target::*;\n",
    );
    fixture.write(
        "glob_facade/recursive.rs",
        "use crate::graph::compiler::GraphCompiler as Unique;\npub use crate::glob_facade::*;\n",
    );
    fixture.write("glob_facade/target.rs", "pub struct Unique;\n");
    fixture.write("graph/mod.rs", "pub mod compiler;\n");
    fixture.write("graph/compiler.rs", "pub struct GraphCompiler;\n");
    fixture.write("sci/src/lib.rs", "pub mod facade;\npub mod api;\n");
    fixture.write(
        "sci/src/facade.rs",
        "pub use crate::api::computation::StatisticalInput;\n",
    );
    fixture.write("sci/src/api/mod.rs", "pub mod computation;\n");
    fixture.write(
        "sci/src/api/computation.rs",
        "pub struct StatisticalInput;\n",
    );

    let fixture_root =
        std::fs::canonicalize(&fixture.root).expect("fixture root must be canonical");
    let member_root = std::fs::canonicalize(fixture.source_root.join("sci/src/lib.rs"))
        .expect("member root must be canonical");
    let workspace = RustWorkspaceModel {
        repository_root: fixture_root.clone(),
        roots: vec![
            ProductionRoot {
                package_id: "fixture-package".to_owned(),
                package: "fixture".to_owned(),
                target: "fixture_lib".to_owned(),
                kind: ProductionRootKind::Library,
                source_path: fixture.source_root.join("lib.rs"),
            },
            ProductionRoot {
                package_id: "sci-package".to_owned(),
                package: "yss-sci".to_owned(),
                target: "yss_sci".to_owned(),
                kind: ProductionRootKind::Library,
                source_path: member_root.clone(),
            },
            ProductionRoot {
                package_id: "fixture-package".to_owned(),
                package: "fixture".to_owned(),
                target: "fixture".to_owned(),
                kind: ProductionRootKind::Binary,
                source_path: fixture.source_root.join("main.rs"),
            },
        ],
        dependency_declarations: vec![
            CargoDependencyDeclaration {
                owning_package_id: "fixture-package".to_owned(),
                owning_package: "fixture".to_owned(),
                declared_name: "science_api".to_owned(),
                package_name: "yss-sci".to_owned(),
                authority: CargoDependencyAuthority::WorkspaceMember {
                    member_package_id: "sci-package".to_owned(),
                },
                scope: CargoDependencyScope::Runtime,
                target_condition: None,
            },
            CargoDependencyDeclaration {
                owning_package_id: "fixture-package".to_owned(),
                owning_package: "fixture".to_owned(),
                declared_name: "polars".to_owned(),
                package_name: "polars".to_owned(),
                authority: CargoDependencyAuthority::External,
                scope: CargoDependencyScope::Runtime,
                target_condition: None,
            },
            CargoDependencyDeclaration {
                owning_package_id: "fixture-package".to_owned(),
                owning_package: "fixture".to_owned(),
                declared_name: "tauri_build".to_owned(),
                package_name: "tauri-build".to_owned(),
                authority: CargoDependencyAuthority::External,
                scope: CargoDependencyScope::Build,
                target_condition: None,
            },
            CargoDependencyDeclaration {
                owning_package_id: "fixture-package".to_owned(),
                owning_package: "fixture".to_owned(),
                declared_name: "ndarray".to_owned(),
                package_name: "ndarray".to_owned(),
                authority: CargoDependencyAuthority::External,
                scope: CargoDependencyScope::Runtime,
                target_condition: None,
            },
        ],
        workspace_member_crate_aliases: vec![WorkspaceMemberCrateAlias {
            owning_package_id: "fixture-package".to_owned(),
            owning_package: "fixture".to_owned(),
            declared_name: "science_api".to_owned(),
            member_package_id: "sci-package".to_owned(),
            member_package: "yss-sci".to_owned(),
            library_crate_name: "yss_sci".to_owned(),
            library_root: member_root,
            root_owner: "yss_sci".to_owned(),
        }],
    };
    let raw = vec![
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::ReExport,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::GraphCompiler".to_owned(),
            line: 1,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "std::sync::Arc".to_owned(),
            line: 2,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/build.rs".to_owned(),
            fully_qualified_owner: "build_script_build".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Build,
            written_target: "tauri_build::build".to_owned(),
            line: 1,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Use,
            mode: RustDependencyMode::Runtime,
            written_target: "science_api::facade::StatisticalInput".to_owned(),
            line: 3,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::ReExport,
            mode: RustDependencyMode::Runtime,
            written_target: "local::Local".to_owned(),
            line: 2,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "Array1::zeros".to_owned(),
            line: 4,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "Column::from".to_owned(),
            line: 5,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Attribute,
            mode: RustDependencyMode::Runtime,
            written_target: "facade/local.rs".to_owned(),
            line: 6,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "f64::max".to_owned(),
            line: 7,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::Local::new".to_owned(),
            line: 8,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::ReExport,
            mode: RustDependencyMode::Runtime,
            written_target: "local::*".to_owned(),
            line: 9,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::Included".to_owned(),
            line: 10,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::glob_facade::Unique".to_owned(),
            line: 11,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::Generated".to_owned(),
            line: 12,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::GeneratedId".to_owned(),
            line: 13,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::GeneratedSemantic".to_owned(),
            line: 14,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::ExecutionDemand".to_owned(),
            line: 15,
            column: 1,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/main.rs".to_owned(),
            fully_qualified_owner: "fixture".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "fixture_lib::facade::GraphCompiler".to_owned(),
            line: 1,
            column: 13,
        },
        RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade/child.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade::child".to_owned(),
            kind: RustDependencyKind::Use,
            mode: RustDependencyMode::Runtime,
            written_target: "super::PrivateCompiler".to_owned(),
            line: 1,
            column: 5,
        },
    ];

    let canonical = super::resolver::resolve_canonical_dependencies(&workspace, &raw)
        .expect("canonical dependency resolution must complete");
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                package_name,
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
            } if package_name == "fixture"
                && repository_relative_declaration_file == "src/graph/compiler.rs"
                && fully_qualified_target == "fixture_lib::graph::compiler::GraphCompiler"
                && symbol == "GraphCompiler"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                package_name,
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
            } if package_name == "fixture"
                && repository_relative_declaration_file == "src/facade/local.rs"
                && fully_qualified_target == "fixture_lib::facade::local::Local"
                && symbol == "Local"
        )
    }));
    assert!(
        canonical.iter().any(|dependency| {
            dependency.origin
                == CanonicalOrigin::External(ExternalDependencyOrigin {
                    declared_name: "ndarray".to_owned(),
                    package_name: "ndarray".to_owned(),
                    declaration_scope: CargoDependencyScope::Runtime,
                    target_condition: None,
                    canonical_subpath: Some("Array1::zeros".to_owned()),
                })
        }),
        "canonical dependencies were {canonical:#?}"
    );
    assert!(canonical.iter().any(|dependency| {
        dependency.origin
            == CanonicalOrigin::External(ExternalDependencyOrigin {
                declared_name: "polars".to_owned(),
                package_name: "polars".to_owned(),
                declaration_scope: CargoDependencyScope::Runtime,
                target_condition: None,
                canonical_subpath: Some("prelude::Column::from".to_owned()),
            })
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.origin
            == CanonicalOrigin::RepositoryAsset {
                repository_relative_path: "src/facade/local.rs".to_owned(),
            }
            && dependency.canonical_origin_target == "repository-asset:src/facade/local.rs"
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade/local.rs"
                && fully_qualified_target == "fixture_lib::facade::local::Local::new"
                && symbol == "Local::new"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade/local.rs"
                && fully_qualified_target == "fixture_lib::facade::local::*"
                && symbol == "*"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade/included.rs"
                && fully_qualified_target == "fixture_lib::facade::Included"
                && symbol == "Included"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/glob_facade/target.rs"
                && fully_qualified_target == "fixture_lib::glob_facade::target::Unique"
                && symbol == "Unique"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade.rs"
                && fully_qualified_target == "fixture_lib::facade::Generated"
                && symbol == "Generated"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade.rs"
                && fully_qualified_target == "fixture_lib::facade::GeneratedId"
                && symbol == "GeneratedId"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade.rs"
                && fully_qualified_target == "fixture_lib::facade::GeneratedSemantic"
                && symbol == "GeneratedSemantic"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
                ..
            } if repository_relative_declaration_file == "src/facade.rs"
                && fully_qualified_target == "fixture_lib::facade::ExecutionDemand"
                && symbol == "ExecutionDemand"
        )
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.source_file == "src/main.rs"
            && matches!(
                &dependency.origin,
                CanonicalOrigin::Repository {
                    fully_qualified_target,
                    symbol,
                    ..
                } if fully_qualified_target == "fixture_lib::graph::compiler::GraphCompiler"
                    && symbol == "GraphCompiler"
            )
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.source_file == "src/facade/child.rs"
            && matches!(
                &dependency.origin,
                CanonicalOrigin::Repository {
                    fully_qualified_target,
                    symbol,
                    ..
                } if fully_qualified_target == "fixture_lib::graph::compiler::GraphCompiler"
                    && symbol == "GraphCompiler"
            )
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.origin
            == CanonicalOrigin::LanguageBuiltin {
                crate_name: "core".to_owned(),
                canonical_subpath: Some("primitive::f64::max".to_owned()),
            }
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.origin
            == CanonicalOrigin::LanguageBuiltin {
                crate_name: "std".to_owned(),
                canonical_subpath: Some("sync::Arc".to_owned()),
            }
    }));
    assert!(canonical.iter().any(|dependency| {
        dependency.origin
            == CanonicalOrigin::External(ExternalDependencyOrigin {
                declared_name: "tauri_build".to_owned(),
                package_name: "tauri-build".to_owned(),
                declaration_scope: CargoDependencyScope::Build,
                target_condition: None,
                canonical_subpath: Some("build".to_owned()),
            })
    }));
    assert!(canonical.iter().any(|dependency| {
        matches!(
            &dependency.origin,
            CanonicalOrigin::Repository {
                package_name,
                repository_relative_declaration_file,
                fully_qualified_target,
                symbol,
            } if package_name == "yss-sci"
                && repository_relative_declaration_file == "src/sci/src/api/computation.rs"
                && fully_qualified_target == "yss_sci::api::computation::StatisticalInput"
                && symbol == "StatisticalInput"
        )
    }));

    let local_owner_error = super::resolver::resolve_canonical_dependencies(
        &workspace,
        &[RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/facade.rs".to_owned(),
            fully_qualified_owner: "fixture_lib::facade".to_owned(),
            kind: RustDependencyKind::Path,
            mode: RustDependencyMode::Runtime,
            written_target: "local::Missing".to_owned(),
            line: 10,
            column: 1,
        }],
    )
    .expect_err("a declared module owner must not be rebound through its own glob import");
    assert!(matches!(
        local_owner_error,
        ArchitectureAuditError::UnknownDependencyTarget { .. }
    ));

    let mut development_workspace = workspace.clone();
    development_workspace
        .dependency_declarations
        .push(CargoDependencyDeclaration {
            owning_package_id: "fixture-package".to_owned(),
            owning_package: "fixture".to_owned(),
            declared_name: "dev_api".to_owned(),
            package_name: "syn".to_owned(),
            authority: CargoDependencyAuthority::External,
            scope: CargoDependencyScope::Development,
            target_condition: None,
        });
    let error = super::resolver::resolve_canonical_dependencies(
        &development_workspace,
        &[RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Use,
            mode: RustDependencyMode::Runtime,
            written_target: "dev_api::Thing".to_owned(),
            line: 4,
            column: 1,
        }],
    )
    .expect_err("development dependencies must not authorize production imports");
    assert!(matches!(
        error,
        ArchitectureAuditError::DevelopmentDependencyInProduction { .. }
    ));

    let unknown_error = super::resolver::resolve_canonical_dependencies(
        &workspace,
        &[RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Use,
            mode: RustDependencyMode::Runtime,
            written_target: "unknown_api::Thing".to_owned(),
            line: 5,
            column: 1,
        }],
    )
    .expect_err("undeclared package aliases must fail closed");
    assert!(matches!(
        unknown_error,
        ArchitectureAuditError::UnknownDependencyTarget { .. }
    ));

    let private_facade_error = super::resolver::resolve_canonical_dependencies(
        &workspace,
        &[RawDependency {
            owning_package: "fixture".to_owned(),
            repository_relative_source_file: "src/lib.rs".to_owned(),
            fully_qualified_owner: "fixture_lib".to_owned(),
            kind: RustDependencyKind::Use,
            mode: RustDependencyMode::Runtime,
            written_target: "crate::facade::PrivateCompiler".to_owned(),
            line: 6,
            column: 1,
        }],
    )
    .expect_err("private imports must not become public facade exports");
    assert!(matches!(
        private_facade_error,
        ArchitectureAuditError::UnresolvedRepositoryTarget { .. }
    ));
}

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::audit_production_dependency;

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
    fixture.write("project/included_body.rs", "fn included_body() {}\n");

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

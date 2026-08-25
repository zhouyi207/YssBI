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

#[path = "redirected.rs"]
mod redirected;

#[cfg(test)]
mod tests;

#[cfg(all(test, windows))]
mod windows_tests;

#[cfg(any(test, feature = "fixture"))]
mod mixed;

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

fn invoke() {
    application::run();
    matches!((), crate::application::MacroPattern);
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

    let violations = audit_production_dependency(&fixture.source_root, "project", "application")
        .expect("fixture dependency audit must complete");
    let files = violations
        .iter()
        .map(|violation| violation.file.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        files,
        BTreeSet::from([
            "project/mixed.rs",
            "project/production.rs",
            "project/redirected.rs",
        ])
    );

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
}

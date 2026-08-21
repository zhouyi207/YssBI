use std::path::{Path, PathBuf};

use super::catalog::{
    CompilerDiagnosticAudit, audit_compiler_diagnostic_tree, inspect_compiler_diagnostic_source,
};
use super::graph::{audit_production_graph_write_surface, audit_raw_graph_document_mutations};
use super::registry::audit_source_tree;

#[test]
fn compiler_diagnostic_audit_detects_detail_only_as_an_argument_map_key() {
    let source = r#"
fn build(value: Box<str>) {
    let _unrelated = "detail";
    let _from_alias = DiagnosticArguments::from([(Box::<str>::from("detail"), value.clone())]);
    let _from_map = BTreeMap::from([("detail".into(), value.clone())]);
    let mut typed: DiagnosticArguments = DiagnosticArguments::new();
    typed.insert(Box::from("detail"), value.clone());
    let mut plain: BTreeMap<Box<str>, Box<str>> = BTreeMap::new();
    plain.insert("detail".to_owned(), value);
    unrelated.insert("detail", 1);
}
"#;
    let mut audit = CompilerDiagnosticAudit::default();
    inspect_compiler_diagnostic_source("detail.rs", source, &mut audit);

    let detail_violations = audit
        .violations
        .iter()
        .filter(|violation| violation.contains("generic compiler diagnostic argument"))
        .count();
    assert_eq!(detail_violations, 4, "{:#?}", audit.violations);
}

#[test]
fn compiler_diagnostic_constructor_flow_resolves_typed_local_receivers() {
    let source = r#"
struct Problem {
    code: &'static str,
    detail: String,
}

struct Factory;
impl Factory {
    fn new() -> Self {
        Self
    }

    fn unrelated() -> OtherFactory {
        OtherFactory
    }

    fn make(stable_id: &'static str) -> Problem {
        Problem { code: stable_id, detail: String::new() }
    }
}

struct OtherFactory;
impl OtherFactory {
    fn make(stable_id: &'static str) -> usize {
        stable_id.len()
    }
}

fn forwarded_from_path(stable_id: &'static str) -> Problem {
    let factory = Factory;
    factory.make(stable_id)
}

fn forwarded_from_proven_return(stable_id: &'static str) -> Problem {
    let factory = Factory::new();
    factory.make(stable_id)
}

fn forwarded_from_annotation(stable_id: &'static str) -> Problem {
    let factory: Factory = opaque_factory();
    factory.make(stable_id)
}

fn unrelated_associated_result(stable_id: &'static str) -> usize {
    let factory = Factory::unrelated();
    factory.make(stable_id)
}

fn emit() {
    let _ = forwarded_from_path("compiler.local_receiver.path");
    let _ = forwarded_from_proven_return("compiler.local_receiver.proven_return");
    let _ = forwarded_from_annotation("compiler.local_receiver.annotation");
    let _ = unrelated_associated_result("compiler.local_receiver.ambiguous_noise");
}
"#;
    let mut audit = CompilerDiagnosticAudit::default();
    inspect_compiler_diagnostic_source("local_receiver.rs", source, &mut audit);

    assert!(
        audit.violations.iter().any(|violation| violation
            .contains("generic compiler diagnostic constructor:fn forwarded_from_annotation")),
        "forwarding constructor was not identified: {:#?}",
        audit.violations
    );
}

#[test]
fn compiler_diagnostic_audit_detects_structural_issue_and_constructor_forms() {
    let source = r#"
struct Problem {
    code: &'static str,
    detail: String,
}

struct UnrelatedResponse {
    code: u16,
    detail: String,
}

fn free(stable_id: &'static str, detail: String) -> Problem {
    Problem {
        code: stable_id,
        detail,
    }
}

struct Factory;
impl Factory {
    fn inherent(stable_id: &'static str, detail: String) -> NodeDiagnostic {
        NodeDiagnostic {
            code: DiagnosticCode::new(stable_id),
            detail,
        }
    }
}

trait BuildsProblem {
    fn trait_method(stable_id: &'static str, detail: String) -> Problem {
        Problem {
            code: stable_id,
            detail,
        }
    }
}

fn unrelated(code: &'static str) -> usize {
    code.len()
}
"#;
    let mut audit = CompilerDiagnosticAudit::default();
    inspect_compiler_diagnostic_source("structural.rs", source, &mut audit);

    for expected in [
        "untyped compiler issue field:code",
        "untyped compiler issue field:detail",
        "generic compiler diagnostic constructor:fn free",
        "generic compiler diagnostic constructor:fn inherent",
        "generic compiler diagnostic constructor:fn trait_method",
        "direct compiler NodeDiagnostic construction:NodeDiagnostic {",
    ] {
        assert!(
            audit
                .violations
                .iter()
                .any(|violation| violation.contains(expected)),
            "missing {expected} in {:#?}",
            audit.violations
        );
    }
    assert!(
        audit
            .violations
            .iter()
            .all(|violation| !violation.contains("fn unrelated")),
        "unrelated code parameter was classified as diagnostic: {:#?}",
        audit.violations
    );
    assert_eq!(
        audit
            .violations
            .iter()
            .filter(|violation| violation.contains("untyped compiler issue field"))
            .count(),
        2,
        "non-diagnostic code/detail fields were classified as an issue: {:#?}",
        audit.violations
    );
}

#[test]
fn compiler_diagnostic_audit_resolves_import_and_reexport_aliases() {
    let source = r#"
use crate::node_system::analysis::NodeDiagnostic as ImportedDiagnostic;
pub use crate::node_system::analysis::NodeDiagnostic as ReexportedDiagnostic;

struct UnrelatedDiagnostic {
    code: &'static str,
}

fn emit_import_alias() {
    let _ = ImportedDiagnostic {
        code: DiagnosticCode::new("compiler.alias.import"),
    };
}

fn emit_reexport_alias() {
    let _ = ReexportedDiagnostic {
        code: DiagnosticCode::new("compiler.alias.reexport"),
    };
}

fn unrelated() {
    let _ = UnrelatedDiagnostic { code: "not-a-diagnostic" };
}
"#;
    let mut audit = CompilerDiagnosticAudit::default();
    inspect_compiler_diagnostic_source("aliases.rs", source, &mut audit);

    assert!(
        audit
            .violations
            .iter()
            .any(|violation| violation.contains("ImportedDiagnostic {")),
        "import alias bypassed direct-construction audit: {:#?}",
        audit.violations
    );
    assert!(
        audit
            .violations
            .iter()
            .any(|violation| violation.contains("ReexportedDiagnostic {")),
        "re-export alias bypassed direct-construction audit: {:#?}",
        audit.violations
    );
    assert!(
        audit
            .violations
            .iter()
            .all(|violation| !violation.contains("UnrelatedDiagnostic")),
        "unrelated struct was classified as NodeDiagnostic: {:#?}",
        audit.violations
    );
}

#[test]
fn compiler_diagnostic_audit_recurses_and_excludes_only_authority_and_tests() {
    let root = audit_fixture("compiler-diagnostic-tree");
    write_fixture(
        &root,
        "nested/emitter.rs",
        r#"
fn diagnostic(code: &'static str) -> NodeDiagnostic {
    NodeDiagnostic { code }
}
fn emit() { let _ = diagnostic("compiler.nested.emitted"); }
#[cfg(test)]
fn fixture() {
    let _ = diagnostic("compiler.inline.test");
    let _ = NodeDiagnostic { code: "compiler.inline.direct" };
}
"#,
    );
    write_fixture(
        &root,
        "diagnostics.rs",
        r#"
const KEY: &str = "diagnostics.compiler.authority";
fn authority() { let _ = "compiler.authority.internal"; }
"#,
    );
    write_fixture(
        &root,
        "tests.rs",
        r#"
fn diagnostic(code: &'static str) -> NodeDiagnostic { NodeDiagnostic { code } }
fn fixture() { let _ = diagnostic("compiler.file.test"); }
"#,
    );

    let enforcement = audit_compiler_diagnostic_tree(&root, true);
    assert!(
        enforcement
            .violations
            .iter()
            .any(|violation| violation.contains("compiler.nested.emitted")),
        "nested production file was skipped: {:#?}",
        enforcement.violations
    );
    assert!(
        enforcement
            .violations
            .iter()
            .all(|violation| !violation.contains("authority") && !violation.contains("test")),
        "authority or tests leaked into enforcement: {:#?}",
        enforcement.violations
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn raw_graph_document_audit_rejects_nested_production_declarations() {
    let violations = audit_raw_graph_document_mutations(
        r#"
#[cfg(test)]
impl GraphDocument {
    pub(crate) fn create_node(&mut self) {}
    pub(crate) fn delete_node(&mut self) {}
    pub(crate) fn bind_port(&mut self) {}
    pub(crate) fn connect(&mut self) {}
    pub(crate) fn disconnect(&mut self) {}
    pub(crate) fn set_literal(&mut self) {}
}

#[cfg(not(test))]
mod nested {
    impl GraphDocument {
        fn create_node(&mut self) {}
    }
}
"#,
    );

    assert!(
        violations.iter().any(|violation| violation
            .contains("production GraphDocument impl exposes raw mutation:create_node")),
        "nested production GraphDocument declaration escaped the audit:\n{}",
        violations.join("\n")
    );
}

#[test]
fn raw_graph_document_audit_allows_strict_test_only_ancestor_scopes() {
    let violations = audit_raw_graph_document_mutations(
        r#"
#[cfg(test)]
impl GraphDocument {
    pub(crate) fn create_node(&mut self) {}
    pub(crate) fn delete_node(&mut self) {}
    pub(crate) fn bind_port(&mut self) {}
    pub(crate) fn connect(&mut self) {}
    pub(crate) fn disconnect(&mut self) {}
    pub(crate) fn set_literal(&mut self) {}
}

#[cfg(test)]
mod fixture_module {
    fn calls(document: &mut GraphDocument) {
        document.create_node();
        let raw = GraphDocument::delete_node;
        raw(document);
    }
}

struct Fixture;

#[cfg(test)]
impl Fixture {
    fn calls(document: &mut GraphDocument) {
        document.bind_port();
    }
}

impl Fixture {
    #[cfg(test)]
    fn method(document: &mut GraphDocument) {
        document.connect();
    }

    fn scoped(document: &mut GraphDocument) {
        #[cfg(test)]
        {
            document.disconnect();
        }

        #[cfg(test)]
        GraphDocument::set_literal(document);

        #[cfg(test)]
        let raw = GraphDocument::create_node;
        #[cfg(test)]
        raw(document);
    }
}
"#,
    );

    assert!(
        violations.is_empty(),
        "strict test-only ancestor scopes produced false positives:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_graph_document_exposes_no_raw_mutation_methods() {
    let bypasses = audit_raw_graph_document_mutations(
        r#"
use crate::node_system::document::GraphDocument;

fn method_call(document: &mut GraphDocument) {
    document.create_node(todo!()).unwrap();
}

fn ufcs(document: &mut GraphDocument) {
    GraphDocument::delete_node(document, todo!()).unwrap();
}

fn alias(document: &mut GraphDocument) {
    let raw = GraphDocument::bind_port;
    raw(document, todo!(), todo!()).unwrap();
}

#[cfg(any(test, feature = "fixture"))]
fn weak_call(document: &mut GraphDocument) {
    document.set_literal(todo!(), todo!()).unwrap();
}

#[cfg(any(test, feature = "fixture"))]
impl GraphDocument {
    pub(crate) fn connect(&mut self) {}
}

#[cfg(test)]
impl GraphDocument {
    pub(crate) fn create_node(&mut self) {}
    pub(crate) fn delete_node(&mut self) {}
    pub(crate) fn bind_port(&mut self) {}
    pub(crate) fn disconnect(&mut self) {}
    pub(crate) fn set_literal(&mut self) {}
}
"#,
    );
    for expected in [
        "method call:create_node",
        "UFCS or alias reference:delete_node",
        "UFCS or alias reference:bind_port",
        "method call:set_literal",
        "production GraphDocument impl exposes raw mutation:connect",
    ] {
        assert!(
            bypasses
                .iter()
                .any(|violation| violation.contains(expected)),
            "raw GraphDocument mutation audit missed {expected}:\n{}",
            bypasses.join("\n")
        );
    }

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let violations = audit_raw_graph_document_mutations(
        &std::fs::read_to_string(source_root.join("node_system/document/transaction.rs")).unwrap(),
    );

    assert!(
        violations.is_empty(),
        "production GraphDocument raw mutation violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_graph_write_surface_exposes_only_editor_mutations() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let project_state_source = format!(
        "{}\n{}",
        std::fs::read_to_string(source_root.join("project/project_state.rs")).unwrap(),
        std::fs::read_to_string(source_root.join("project/project_state/editor_mutation.rs"))
            .unwrap(),
    );
    let violations = audit_production_graph_write_surface(
        &std::fs::read_to_string(source_root.join("node_system/document/mod.rs")).unwrap(),
        &std::fs::read_to_string(source_root.join("node_system/document/mutation.rs")).unwrap(),
        &project_state_source,
    );

    assert!(
        violations.is_empty(),
        "production graph write-surface violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_graph_write_surface_audit_rejects_cfg_bypasses() {
    let violations = audit_production_graph_write_surface(
        r#"
pub use patch::GraphDocumentPatch;
#[cfg(not(test))]
pub use mutation::GraphMutation;
#[cfg(any(test, feature = "fixture"))]
pub use mutation::RevisionedGraphStore;
"#,
        r#"
#[cfg(not(test))]
pub fn apply_mutation() {}
"#,
        r#"
pub struct ProjectState;
impl ProjectState {
    pub fn apply_editor_graph_mutation(&self) {}
    #[cfg(not(test))]
    pub fn apply_graph_mutation(&self) {}
    #[cfg(any(test, feature = "fixture"))]
    pub fn apply_graph_patch(&self) {}
}
"#,
    );

    for expected in [
        "raw graph write symbol GraphMutation",
        "raw graph write symbol RevisionedGraphStore",
        "public production free function named apply_mutation",
        "ProjectState::apply_graph_mutation",
        "ProjectState::apply_graph_patch",
    ] {
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(expected)),
            "missing {expected} violation in:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn production_graph_write_surface_audit_allows_exclusive_test_gates() {
    let violations = audit_production_graph_write_surface(
        r#"
pub use patch::GraphDocumentPatch;
#[cfg(test)]
pub use mutation::GraphMutation;
#[cfg(all(feature = "fixture", test))]
pub use mutation::RevisionedGraphStore;
#[cfg(test)]
pub use fixture_exports::*;
"#,
        r#"
#[cfg(any(test, all(test, feature = "fixture")))]
pub fn apply_mutation() {}
"#,
        r#"
pub struct ProjectState;
impl ProjectState {
    pub fn apply_editor_graph_mutation(&self) {}
    #[cfg(test)]
    pub fn apply_graph_mutation(&self) {}
    #[cfg(all(test, feature = "fixture"))]
    pub fn apply_graph_patch(&self) {}
}
"#,
    );

    assert!(
        violations.is_empty(),
        "exclusive test gates must remain fixture-only:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_graph_write_surface_audit_rejects_mutation_glob_reexports() {
    let violations = audit_production_graph_write_surface(
        r#"
pub use patch::GraphDocumentPatch;
pub use mutation::*;
"#,
        "",
        r#"
pub struct ProjectState;
impl ProjectState {
    pub fn apply_editor_graph_mutation(&self) {}
}
"#,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("public glob re-export from mutation")),
        "missing mutation glob violation in:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_graph_write_surface_audit_rejects_indirect_glob_reexports() {
    let violations = audit_production_graph_write_surface(
        r#"
pub use patch::GraphDocumentPatch;
mod exports {
    pub use super::mutation::GraphMutation;
}
pub use exports::*;
"#,
        "",
        r#"
pub struct ProjectState;
impl ProjectState {
    pub fn apply_editor_graph_mutation(&self) {}
}
"#,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("production public glob re-export")),
        "missing indirect glob violation in:\n{}",
        violations.join("\n")
    );
}

#[test]
fn audit_scans_every_rust_file_without_test_filename_exclusions() {
    let root = audit_fixture("scope");

    write_fixture(
        &root,
        "misnamed/production_tests.rs",
        "type NodeRegistry = std::collections::BTreeMap<String, String>;",
    );

    let offenders = audit_source_tree(&root, None);

    assert_offender(
        &offenders,
        "misnamed/production_tests.rs",
        "type alias NodeRegistry",
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_label_based_node_type_construction() {
    let root = audit_fixture("label-identity");
    write_fixture(
        &root,
        "identity.rs",
        r#"fn identity(category: &[String], name: &str) -> NodeTypeId {
    NodeTypeId::new(format!("{}:{}", category.join(":"), name))
}"#,
    );
    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "identity.rs", "category/name identity");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn display_name_pin_audit_distinguishes_pin_identity_from_other_definitions() {
    let root = audit_fixture("display-name-pin-matching");
    write_fixture(
        &root,
        "pin_identity.rs",
        "fn unsupported(pin: Pin) { let _ = pin.definition.name; }",
    );
    write_fixture(
        &root,
        "command_metadata.rs",
        "fn command(definition: TauriCommandDefinition) { let _ = definition.name; }",
    );

    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "pin_identity.rs", "display-name pin matching");
    assert!(
        offenders
            .iter()
            .all(|offender| !offender.starts_with("command_metadata.rs:")),
        "non-pin definition names must not be classified as pin identity:\n{}",
        offenders.join("\n")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_node_registry_and_identity_ast_variants() {
    let root = audit_fixture("ast-variants");

    write_fixture(
        &root,
        "registry_items.rs",
        r#"pub struct
NodeRegistry
{
    value: usize,
}"#,
    );
    write_fixture(
        &root,
        "registry_alias.rs",
        r#"pub type
NodeRegistry
= std::collections::BTreeMap<String, String>;"#,
    );

    write_fixture(
        &root,
        "identity.rs",
        r#"fn identities(category: &[String], categories: &[String], name: &str, title: &str) {
    let node_type_id = NodeTypeId::new(format!("{category:?}:{name}"));
    registry.insert(format!("{}:{}", categories.join(":"), title), node_type_id);
}"#,
    );

    let offenders = audit_source_tree(&root, None);
    for (path, label) in [
        ("registry_items.rs", "second NodeRegistry definition"),
        ("registry_alias.rs", "type alias NodeRegistry"),
        ("identity.rs", "category/name identity"),
    ] {
        assert_offender(&offenders, path, label);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_union_node_registry_definition() {
    let root = audit_fixture("structural-variants");
    write_fixture(&root, "union.rs", "pub union NodeRegistry { value: usize }");

    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "union.rs", "second NodeRegistry definition");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_macro_generated_node_registry() {
    let root = audit_fixture("macro-generated");
    write_fixture(
        &root,
        "generated.rs",
        r#"macro_rules! duplicate_registry {
    () => { pub struct NodeRegistry; };
}
duplicate_registry!();"#,
    );
    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "generated.rs", "macro NodeRegistry definition");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_allows_category_name_breadcrumbs_and_logs() {
    let root = audit_fixture("legal-identity-uses");
    write_fixture(
        &root,
        "legal.rs",
        r#"fn breadcrumb(category: &[String], name: &str) -> String {
    let path = category.join(":");
    tracing::debug!("category path {} for {}", path, name);
    format!("{}:{}", category.join(":"), name)
}"#,
    );

    let offenders = audit_source_tree(&root, None);
    assert!(
        offenders.is_empty(),
        "legal breadcrumbs and logs must not be rejected:\n{}",
        offenders.join("\n")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_real_category_name_identity_sinks() {
    let root = audit_fixture("identity-sinks");
    write_fixture(
        &root,
        "constructor.rs",
        r#"fn identity(category: &[String], name: &str) {
    let node_type_id = NodeTypeId::new(format!("{}:{}", category.join(":"), name));
}"#,
    );
    write_fixture(
        &root,
        "registry.rs",
        r#"fn identity(category: &[String], name: &str, registry: &mut Registry) {
    registry.insert(format!("{category:?}:{name}"), ());
}"#,
    );
    write_fixture(
        &root,
        "assignment.rs",
        r#"fn identity(category: &[String], name: &str) {
    let node_type: String = format!("{}:{}", category.join(":"), name);
}"#,
    );
    write_fixture(
        &root,
        "field_assignment.rs",
        r#"fn identity(category: &[String], name: &str) -> Node {
    Node { node_type: format!("{}:{}", category.join(":"), name) }
}"#,
    );
    write_fixture(
        &root,
        "registry_key.rs",
        r#"fn identity(category: &[String], name: &str, registry: &mut Registry) {
    registry[format!("{}:{}", category.join(":"), name)] = ();
}"#,
    );
    write_fixture(
        &root,
        "return.rs",
        r#"fn node_type(category: &[String], name: &str) -> String {
    format!("{}:{}", category.join(":"), name)
}"#,
    );
    write_fixture(
        &root,
        "method_receiver.rs",
        r#"fn identity(category: &[String], name: &str) {
    let node_type_id = format!("{}:{}", category.join(":"), name).into_boxed_str();
}"#,
    );
    write_fixture(
        &root,
        "binary.rs",
        r#"fn identity(category: &[String], name: &str) {
    let node_type_id = category.join(":") + ":" + name;
}"#,
    );
    write_fixture(
        &root,
        "intermediate_bindings.rs",
        r#"fn identity(category: &[String], name: &str) {
    let prefix = category.join(":");
    let qualified = prefix + ":";
    let candidate = qualified + name;
    let node_type_id = candidate;
}"#,
    );

    let offenders = audit_source_tree(&root, None);
    for path in [
        "constructor.rs",
        "registry.rs",
        "assignment.rs",
        "field_assignment.rs",
        "registry_key.rs",
        "return.rs",
        "method_receiver.rs",
        "binary.rs",
        "intermediate_bindings.rs",
    ] {
        assert_offender(&offenders, path, "category/name identity");
    }
    std::fs::remove_dir_all(root).unwrap();
}

fn audit_fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../target/source-audit-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_fixture(root: &Path, relative: &str, source: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}

fn assert_offender(offenders: &[String], path: &str, label: &str) {
    assert!(
        offenders
            .iter()
            .any(|offender| offender.contains(path) && offender.contains(label)),
        "missing {path} {label} offender in:\n{}",
        offenders.join("\n")
    );
}

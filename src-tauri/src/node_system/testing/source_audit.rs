use std::path::{Path, PathBuf};

use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, Item, Lit, Macro, Member, Meta, Token, UseTree};

const AUDIT_SOURCE: &str = "node_system/testing/source_audit.rs";
const REGISTRY_AUTHORITY: &str = "node_system/registry/model.rs";
const LEGACY_MODULE_PATHS: &[&str] = &[
    "graph/register/",
    "graph/core/",
    "graph/infer/",
    "execution/context/",
    "execution/engine/",
];

fn rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn record(offenders: &mut Vec<String>, relative: &str, line: usize, label: &str, token: &str) {
    offenders.push(format!("{relative}:{line}:{label}:{token}"));
}

fn line_for(source: &str, token: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(token))
        .map_or(1, |line| line + 1)
}

fn normalized_module_path(value: &str) -> String {
    value.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn is_legacy_module_path(value: &str) -> bool {
    let value = normalized_module_path(value);
    LEGACY_MODULE_PATHS
        .iter()
        .any(|prefix| value.contains(prefix))
}

fn path_label(segments: &[String]) -> Option<(&'static str, &'static str)> {
    let contains_pair = |first: &str, second: &str| {
        segments
            .windows(2)
            .any(|pair| pair[0] == first && pair[1] == second)
    };
    if contains_pair("graph", "register") {
        Some(("graph Registry path", "graph::register"))
    } else if contains_pair("execution", "context") {
        Some(("old execution context path", "execution::context"))
    } else if contains_pair("execution", "engine") {
        Some(("old execution engine path", "execution::engine"))
    } else {
        None
    }
}

fn expand_use_tree(tree: &UseTree, prefix: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            expand_use_tree(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            path.push(name.ident.to_string());
            paths.push(path);
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            path.push(rename.ident.to_string());
            paths.push(path);
        }
        UseTree::Glob(_) => paths.push(prefix.clone()),
        UseTree::Group(group) => {
            for tree in &group.items {
                expand_use_tree(tree, prefix, paths);
            }
        }
    }
}

fn expr_mentions(expr: &Expr, names: &[&str]) -> bool {
    match expr {
        Expr::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| names.contains(&segment.ident.to_string().as_str())),
        Expr::Field(field) => {
            expr_mentions(&field.base, names)
                || matches!(&field.member, Member::Named(name) if names.contains(&name.to_string().as_str()))
        }
        Expr::MethodCall(call) => {
            expr_mentions(&call.receiver, names)
                || call
                    .args
                    .iter()
                    .any(|argument| expr_mentions(argument, names))
        }
        Expr::Call(call) => {
            expr_mentions(&call.func, names)
                || call
                    .args
                    .iter()
                    .any(|argument| expr_mentions(argument, names))
        }
        Expr::Reference(reference) => expr_mentions(&reference.expr, names),
        Expr::Paren(paren) => expr_mentions(&paren.expr, names),
        _ => false,
    }
}

fn macro_arguments(mac: &Macro) -> Option<Punctuated<Expr, Token![,]>> {
    Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(mac.tokens.clone())
        .ok()
}

fn format_builds_category_identity(mac: &Macro) -> bool {
    let Some(arguments) = macro_arguments(mac) else {
        return false;
    };
    let format_string = arguments.first().and_then(|argument| match argument {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.value()),
        _ => None,
    });
    let captured_identity = format_string.is_some_and(|format| {
        (format.contains("{category") || format.contains("{categories"))
            && (format.contains("{name") || format.contains("{title"))
    });
    let positional_category = arguments
        .iter()
        .skip(1)
        .any(|argument| expr_mentions(argument, &["category", "categories"]));
    let positional_label = arguments
        .iter()
        .skip(1)
        .any(|argument| expr_mentions(argument, &["name", "title"]));
    captured_identity || (positional_category && positional_label)
}

fn concat_builds_category_identity(mac: &Macro) -> bool {
    let Some(arguments) = macro_arguments(mac) else {
        return false;
    };
    let literals = arguments.iter().filter_map(|argument| match argument {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.value()),
        _ => None,
    });
    let mut category = false;
    let mut label = false;
    for literal in literals {
        category |= matches!(literal.as_str(), "category" | "categories");
        label |= matches!(literal.as_str(), "name" | "title");
    }
    category && label
}

struct SourceVisitor<'a> {
    relative: &'a str,
    source: &'a str,
    offenders: &'a mut Vec<String>,
}

impl SourceVisitor<'_> {
    fn report(&mut self, label: &str, token: &str) {
        record(
            self.offenders,
            self.relative,
            line_for(self.source, token),
            label,
            token,
        );
    }

    fn inspect_segments(&mut self, segments: Vec<String>) {
        if let Some((label, token)) = path_label(&segments) {
            self.report(label, token);
        }
    }

    fn inspect_macro(&mut self, mac: &Macro) {
        let Some(name) = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
        else {
            return;
        };
        if name == "include" {
            if let Ok(path) = syn::parse2::<syn::LitStr>(mac.tokens.clone()) {
                if is_legacy_module_path(&path.value()) {
                    self.report("legacy module include", "include!");
                }
            }
            return;
        }
        let category_identity = match name.as_str() {
            "format" => format_builds_category_identity(mac),
            "concat" => concat_builds_category_identity(mac),
            _ => false,
        };
        if category_identity {
            self.report("category/name identity", "category:name construction");
        }
    }
}

impl<'ast> Visit<'ast> for SourceVisitor<'_> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let mut paths = Vec::new();
        expand_use_tree(&node.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.inspect_segments(path);
        }
        visit::visit_item_use(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.inspect_segments(
            node.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect(),
        );
        visit::visit_path(self, node);
    }

    fn visit_item(&mut self, node: &'ast Item) {
        match node {
            Item::Struct(item) if item.ident == "NodeRegistry" => {
                if self.relative != REGISTRY_AUTHORITY {
                    self.report("second NodeRegistry definition", "NodeRegistry");
                }
            }
            Item::Enum(item) if item.ident == "NodeRegistry" => {
                if self.relative != REGISTRY_AUTHORITY {
                    self.report("second NodeRegistry definition", "NodeRegistry");
                }
            }
            Item::Trait(item) if item.ident == "NodeRegistry" => {
                if self.relative != REGISTRY_AUTHORITY {
                    self.report("second NodeRegistry definition", "NodeRegistry");
                }
            }
            Item::Type(item) if item.ident == "NodeRegistry" => {
                self.report("type alias NodeRegistry", "NodeRegistry");
            }
            Item::Mod(item) if self.relative == "graph/mod.rs" && item.ident == "register" => {
                self.report("compiled graph register module", "register");
            }
            _ => {}
        }
        visit::visit_item(self, node);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let value = ident.to_string();
        let finding = match value.as_str() {
            "GraphInstance" => Some(("legacy GraphInstance", "GraphInstance")),
            "NodeDefinition" => Some(("legacy node definition", "NodeDefinition")),
            "NodeExecutionContext" => Some(("old execution context type", "NodeExecutionContext")),
            "ExecutionStack" | "ExecutionFrame" | "Executor" => {
                Some(("old execution engine type", value.as_str()))
            }
            "reconcile_node_pins" => Some(("dynamic pin reconciliation", "reconcile_node_pins")),
            "resolve_dynamic_pins" | "resolve_all_dynamic_pins" => {
                Some(("dynamic pin resolution", value.as_str()))
            }
            "sync_static_pin_definitions" => {
                Some(("static pin reconciliation", "sync_static_pin_definitions"))
            }
            "node_type_from_category" | "category_name_identity" | "category_and_name_identity" => {
                Some(("category/name identity", value.as_str()))
            }
            _ => None,
        };
        if let Some((label, token)) = finding {
            self.report(label, token);
        }
        visit::visit_ident(self, ident);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "placeholder")
            {
                self.report("placeholder node definition", "placeholder");
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "placeholder" {
            self.report("placeholder node definition", "placeholder");
        }
        if node.method == "join"
            && expr_mentions(&node.receiver, &["category", "categories"])
            && node.args.len() == 1
            && matches!(node.args.first(), Some(Expr::Lit(ExprLit { lit: Lit::Str(value), .. })) if value.value() == ":")
        {
            self.report("category/name identity", "category.join");
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if matches!(&node.member, Member::Named(name) if name == "name")
            && expr_mentions(&node.base, &["definition", "pin_definition"])
        {
            self.report("display-name pin matching", "pin.definition.name");
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("path") {
            if let Meta::NameValue(meta) = &node.meta {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(path),
                    ..
                }) = &meta.value
                {
                    if is_legacy_module_path(&path.value()) {
                        self.report("legacy module path attribute", "#[path]");
                    }
                }
            }
        }
        visit::visit_attribute(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        self.inspect_macro(node);
        visit::visit_macro(self, node);
    }
}

fn inspect_source(relative: &str, source: &str, offenders: &mut Vec<String>) {
    for prefix in LEGACY_MODULE_PATHS {
        if relative.starts_with(prefix) {
            record(offenders, relative, 1, "orphan legacy source", prefix);
        }
    }

    match syn::parse_file(source) {
        Ok(file) => SourceVisitor {
            relative,
            source,
            offenders,
        }
        .visit_file(&file),
        Err(error) => record(
            offenders,
            relative,
            1,
            "Rust source parse failure",
            &error.to_string(),
        ),
    }
}

fn audit_source_tree(root: &Path, excluded_relative: Option<&str>) -> Vec<String> {
    let mut files = Vec::new();
    rust_sources(root, &mut files);
    files.sort();
    let mut offenders = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if excluded_relative == Some(relative.as_str()) {
            continue;
        }
        let source = std::fs::read_to_string(&file).unwrap();
        inspect_source(&relative, &source, &mut offenders);
    }
    offenders.sort();
    offenders.dedup();
    offenders
}

#[test]
fn production_has_one_node_registry_and_no_label_identity() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let offenders = audit_source_tree(&source_root, Some(AUDIT_SOURCE));
    assert!(
        offenders.is_empty(),
        "legacy Rust architecture violations:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn audit_scans_every_rust_file_without_test_filename_exclusions() {
    let root = audit_fixture("scope");
    write_fixture(&root, "outside/legacy.rs", "pub struct GraphInstance;");
    write_fixture(
        &root,
        "misnamed/production_tests.rs",
        "type NodeRegistry = std::collections::BTreeMap<String, String>;",
    );

    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "outside/legacy.rs", "GraphInstance");
    assert_offender(
        &offenders,
        "misnamed/production_tests.rs",
        "type alias NodeRegistry",
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_grouped_uses_label_construction_and_source_bypasses() {
    let root = audit_fixture("tokens");
    write_fixture(
        &root,
        "grouped.rs",
        "use crate::graph::{value::DataValue, register::NodeRegistry};",
    );
    write_fixture(
        &root,
        "identity.rs",
        r#"fn identity(category: &[String], name: &str) -> String {
    format!("{}:{}", category.join(":"), name)
}"#,
    );
    write_fixture(
        &root,
        "bypass.rs",
        "#[path = \"graph/register/hidden.rs\"] mod hidden; include!(\"execution/engine/more.rs\");",
    );

    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "grouped.rs", "graph Registry path");
    assert_offender(&offenders, "identity.rs", "category/name identity");
    assert_offender(&offenders, "bypass.rs", "legacy module path attribute");
    assert_offender(&offenders, "bypass.rs", "legacy module include");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_ast_variants_reviewers_can_write_normally() {
    let root = audit_fixture("ast-variants");
    write_fixture(
        &root,
        "visibility.rs",
        r#"pub(in crate) use crate::graph::{
    value::DataValue,
    register::{self as old_register, NodeRegistry as OldRegistry},
};"#,
    );
    write_fixture(
        &root,
        "expression_paths.rs",
        r#"fn construct() {
    crate::graph::register::legacy_registry();
    crate::execution::engine::Executor::new();
}"#,
    );
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
        "legacy_symbols.rs",
        r#"fn legacy(graph: GraphInstance, definition: NodeDefinition) {
    reconcile_node_pins();
    resolve_dynamic_pins();
    sync_static_pin_definitions();
    let _ = graph;
    let _ = definition;
}"#,
    );
    write_fixture(
        &root,
        "legacy_modules.rs",
        r#"#[path = "graph/register/registry.rs"]
mod registry;
include!("execution/engine/mod.rs");"#,
    );
    write_fixture(
        &root,
        "identity.rs",
        r#"fn identities(category: &[String], categories: &[String], name: &str, title: &str) {
    let _ = format!("{category:?}:{name}");
    let _ = format!("{}:{}", categories.join(":"), title);
    let _ = concat!("category", ":", "name");
}"#,
    );

    let offenders = audit_source_tree(&root, None);
    for (path, label) in [
        ("visibility.rs", "graph Registry path"),
        ("expression_paths.rs", "graph Registry path"),
        ("expression_paths.rs", "old execution engine path"),
        ("registry_items.rs", "second NodeRegistry definition"),
        ("registry_alias.rs", "type alias NodeRegistry"),
        ("legacy_symbols.rs", "legacy GraphInstance"),
        ("legacy_symbols.rs", "legacy node definition"),
        ("legacy_symbols.rs", "dynamic pin reconciliation"),
        ("legacy_modules.rs", "legacy module path attribute"),
        ("legacy_modules.rs", "legacy module include"),
        ("identity.rs", "category/name identity"),
    ] {
        assert_offender(&offenders, path, label);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_allows_nonlegacy_path_and_include_macros() {
    let root = audit_fixture("legal-module-sources");
    write_fixture(
        &root,
        "legal.rs",
        r#"#[path = "current/generated.rs"]
mod generated;
include!("current/generated_values.rs");"#,
    );

    let offenders = audit_source_tree(&root, None);
    assert!(
        offenders.is_empty(),
        "legal module source paths must not be rejected:\n{}",
        offenders.join("\n")
    );
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

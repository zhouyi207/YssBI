use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::dependency_audit::collect_production_modules;
use super::model::{ArchitectureAuditError, RustLayer, RustModule};
use super::policy::classify_rust_sources;
use crate::test_support::source_audit::is_test_only;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ImplItem, Item, Token, TraitItem, Type, UseTree, Visibility};

const WORKER_FILE: &str = "src-tauri/src/sci/api/bayes/worker.rs";
const JULIA_WORKER_ADAPTER_FILES: &[&str] = &[
    "src-tauri/src/julia/bayes_worker_adapter/mod.rs",
    "src-tauri/src/julia/bayes_worker_adapter/fit.rs",
    "src-tauri/src/julia/bayes_worker_adapter/predictor.rs",
];
const AUTHORITY_METHODS: &[&str] = &[
    "issue_for_worker",
    "mint_for_worker",
    "validated_worker_result",
    "from_worker",
];
const AUTHORITY_TYPES: &[&str] = &[
    "BayesTaskHandle",
    "BayesArtifactHandle",
    "BayesTaskResult",
    "BayesArtifact",
];
const PRIVATE_FIELD_TYPES: &[&str] = &[
    "BayesTaskId",
    "ArtifactId",
    "BayesTaskGeneration",
    "BayesTaskHandle",
    "ValidatedBayesTask",
    "BayesTaskResult",
    "BayesArtifactHandle",
    "BayesArtifact",
];
const WORKER_PRIVATE_FIELDS: &[&str] = &[
    "task_id",
    "generation",
    "task",
    "artifact",
    "model",
    "inputs",
    "inference",
    "artifacts",
    "handle",
    "media_type",
    "bytes",
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BayesWorkerAuthorityViolation {
    source_file: String,
    kind: &'static str,
    target: String,
}

fn bayes_worker_authority_violations(
    repository_root: &Path,
    modules: &[RustModule],
    classification: &BTreeMap<String, RustLayer>,
) -> Result<Vec<BayesWorkerAuthorityViolation>, ArchitectureAuditError> {
    let source_files = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();
    for source_file in source_files {
        let Some(layer) = classification.get(source_file).copied() else {
            continue;
        };
        let path = repository_root.join(source_file);
        let source =
            std::fs::read_to_string(&path).map_err(|source| ArchitectureAuditError::Io {
                path: path.clone(),
                source,
            })?;
        let mut source_violations = bayes_worker_source_violations(source_file, layer, &source)
            .map_err(|source| ArchitectureAuditError::SourceParse {
                path: path.clone(),
                source,
            })?;
        violations.append(&mut source_violations);
    }
    violations.sort();
    Ok(violations)
}

fn bayes_worker_source_violations(
    source_file: &str,
    layer: RustLayer,
    source: &str,
) -> Result<Vec<BayesWorkerAuthorityViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let worker_reference = source.contains("bayes::worker")
        || AUTHORITY_TYPES
            .iter()
            .any(|authority_type| source.contains(authority_type));
    let allow_authority =
        source_file == WORKER_FILE || JULIA_WORKER_ADAPTER_FILES.contains(&source_file);
    let mut violations = Vec::new();
    let mut visitor = BayesWorkerAuthorityVisitor {
        source_file,
        layer,
        worker_reference,
        allow_authority,
        violations: &mut violations,
    };
    visitor.visit_file(&syntax);
    violations.sort();
    Ok(violations)
}

struct BayesWorkerAuthorityVisitor<'a> {
    source_file: &'a str,
    layer: RustLayer,
    worker_reference: bool,
    allow_authority: bool,
    violations: &'a mut Vec<BayesWorkerAuthorityViolation>,
}

impl BayesWorkerAuthorityVisitor<'_> {
    fn record(&mut self, kind: &'static str, target: impl Into<String>) {
        self.violations.push(BayesWorkerAuthorityViolation {
            source_file: self.source_file.to_owned(),
            kind,
            target: target.into(),
        });
    }

    fn layer_forbids_field_access(&self) -> bool {
        matches!(
            self.layer,
            RustLayer::Application
                | RustLayer::Commands
                | RustLayer::Project
                | RustLayer::Graph
                | RustLayer::Execution
        )
    }
}

impl<'ast> Visit<'ast> for BayesWorkerAuthorityVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if is_test_only(item_attributes(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if is_test_only(impl_item_attributes(item)) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if is_test_only(trait_item_attributes(item)) {
            return;
        }
        visit::visit_trait_item(self, item);
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if is_test_only(expr_attributes(expression)) {
            return;
        }
        visit::visit_expr(self, expression);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if is_test_only(&item.attrs) {
            return;
        }
        if use_tree_has_worker_glob(&item.tree, &mut Vec::new()) {
            self.record("broad-import", "worker::*");
        }
        visit::visit_item_use(self, item);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.allow_authority
            && let Expr::Path(path) = call.func.as_ref()
            && let Some(method) = path.path.segments.last()
            && AUTHORITY_METHODS.contains(&method.ident.to_string().as_str())
        {
            self.record("authority-call", method.ident.to_string());
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_struct(&mut self, construction: &'ast syn::ExprStruct) {
        if !self.allow_authority
            && let Some(ty) = construction.path.segments.last()
            && AUTHORITY_TYPES.contains(&ty.ident.to_string().as_str())
        {
            self.record("construction", ty.ident.to_string());
        }
        visit::visit_expr_struct(self, construction);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if !self.allow_authority
            && self.worker_reference
            && self.layer_forbids_field_access()
            && let syn::Member::Named(member) = &field.member
            && WORKER_PRIVATE_FIELDS.contains(&member.to_string().as_str())
        {
            self.record("field-access", member.to_string());
        }
        visit::visit_expr_field(self, field);
    }
}

fn use_tree_has_worker_glob(tree: &UseTree, prefix: &mut Vec<String>) -> bool {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let found = use_tree_has_worker_glob(&path.tree, prefix);
            prefix.pop();
            found
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|item| use_tree_has_worker_glob(item, prefix)),
        UseTree::Glob(_) => prefix.last().is_some_and(|segment| segment == "worker"),
        UseTree::Name(_) | UseTree::Rename(_) => false,
    }
}

fn bayes_worker_surface_violations(
    source: &str,
) -> Result<Vec<BayesWorkerAuthorityViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut violations = Vec::new();
    let mut required_methods = AUTHORITY_METHODS.iter().copied().collect::<BTreeSet<_>>();

    for item in &syntax.items {
        match item {
            Item::Struct(item_struct)
                if PRIVATE_FIELD_TYPES.contains(&item_struct.ident.to_string().as_str()) =>
            {
                if item_struct
                    .fields
                    .iter()
                    .any(|field| !matches!(field.vis, Visibility::Inherited))
                {
                    violations.push(surface_violation(
                        "public-field",
                        item_struct.ident.to_string(),
                    ));
                }
                if has_forgeable_derive(&item_struct.attrs) {
                    violations.push(surface_violation(
                        "forgeable-derive",
                        item_struct.ident.to_string(),
                    ));
                }
            }
            Item::Impl(item_impl) => {
                let Some(owner) = impl_owner(&item_impl.self_ty) else {
                    continue;
                };
                for impl_item in &item_impl.items {
                    let ImplItem::Fn(function) = impl_item else {
                        continue;
                    };
                    let name = function.sig.ident.to_string();
                    if AUTHORITY_METHODS.contains(&name.as_str()) {
                        required_methods.remove(name.as_str());
                        if !is_crate_visibility(&function.vis) {
                            violations.push(surface_violation(
                                "authority-visibility",
                                format!("{owner}::{name}"),
                            ));
                        }
                    }
                    if AUTHORITY_TYPES.contains(&owner.as_str())
                        && matches!(function.vis, Visibility::Public(_))
                        && matches!(
                            name.as_str(),
                            "new"
                                | "issue_for_worker"
                                | "mint_for_worker"
                                | "validated_worker_result"
                                | "from_worker"
                        )
                    {
                        violations.push(surface_violation(
                            "public-constructor",
                            format!("{owner}::{name}"),
                        ));
                    }
                }
            }
            Item::Const(_)
            | Item::Enum(_)
            | Item::ExternCrate(_)
            | Item::Fn(_)
            | Item::ForeignMod(_)
            | Item::Macro(_)
            | Item::Mod(_)
            | Item::Static(_)
            | Item::Trait(_)
            | Item::TraitAlias(_)
            | Item::Type(_)
            | Item::Union(_)
            | Item::Use(_)
            | Item::Verbatim(_) => {}
            _ => {}
        }
    }
    for missing in required_methods {
        violations.push(surface_violation("missing-authority-method", missing));
    }
    violations.sort();
    Ok(violations)
}

fn surface_violation(
    kind: &'static str,
    target: impl Into<String>,
) -> BayesWorkerAuthorityViolation {
    BayesWorkerAuthorityViolation {
        source_file: WORKER_FILE.to_owned(),
        kind,
        target: target.into(),
    }
}

fn impl_owner(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_crate_visibility(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Restricted(restricted) if restricted.path.is_ident("crate"))
}

fn has_forgeable_derive(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("derive") {
            return false;
        }
        attribute
            .parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            .is_ok_and(|derives| {
                derives.iter().any(|derive| {
                    derive.segments.last().is_some_and(|segment| {
                        matches!(
                            segment.ident.to_string().as_str(),
                            "Default" | "Deserialize"
                        )
                    })
                })
            })
    })
}

fn item_attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_item_attributes(item: &ImplItem) -> &[syn::Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_item_attributes(item: &TraitItem) -> &[syn::Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn expr_attributes(expression: &Expr) -> &[syn::Attribute] {
    match expression {
        Expr::Array(expression) => &expression.attrs,
        Expr::Assign(expression) => &expression.attrs,
        Expr::Async(expression) => &expression.attrs,
        Expr::Await(expression) => &expression.attrs,
        Expr::Binary(expression) => &expression.attrs,
        Expr::Block(expression) => &expression.attrs,
        Expr::Break(expression) => &expression.attrs,
        Expr::Call(expression) => &expression.attrs,
        Expr::Cast(expression) => &expression.attrs,
        Expr::Closure(expression) => &expression.attrs,
        Expr::Const(expression) => &expression.attrs,
        Expr::Continue(expression) => &expression.attrs,
        Expr::Field(expression) => &expression.attrs,
        Expr::ForLoop(expression) => &expression.attrs,
        Expr::Group(expression) => &expression.attrs,
        Expr::If(expression) => &expression.attrs,
        Expr::Index(expression) => &expression.attrs,
        Expr::Infer(expression) => &expression.attrs,
        Expr::Let(expression) => &expression.attrs,
        Expr::Lit(expression) => &expression.attrs,
        Expr::Loop(expression) => &expression.attrs,
        Expr::Macro(expression) => &expression.attrs,
        Expr::Match(expression) => &expression.attrs,
        Expr::MethodCall(expression) => &expression.attrs,
        Expr::Paren(expression) => &expression.attrs,
        Expr::Path(expression) => &expression.attrs,
        Expr::Range(expression) => &expression.attrs,
        Expr::RawAddr(expression) => &expression.attrs,
        Expr::Reference(expression) => &expression.attrs,
        Expr::Repeat(expression) => &expression.attrs,
        Expr::Return(expression) => &expression.attrs,
        Expr::Struct(expression) => &expression.attrs,
        Expr::Try(expression) => &expression.attrs,
        Expr::TryBlock(expression) => &expression.attrs,
        Expr::Tuple(expression) => &expression.attrs,
        Expr::Unary(expression) => &expression.attrs,
        Expr::Unsafe(expression) => &expression.attrs,
        Expr::While(expression) => &expression.attrs,
        Expr::Yield(expression) => &expression.attrs,
        _ => &[],
    }
}

#[test]
fn bayes_artifact_authority_can_only_be_minted_at_worker_boundary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every production source must classify exactly once");

    let actual =
        bayes_worker_authority_violations(&workspace.repository_root, &modules, &classification)
            .expect("the Bayes worker authority guard must audit the real source graph");
    assert!(
        actual.is_empty(),
        "Bayes worker authority escaped its exact boundary: {actual:#?}"
    );

    let fixture = bayes_worker_source_violations(
        "src-tauri/src/application/forbidden.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::*;

fn forge(result: BayesTaskResult) {
    let _ = BayesTaskHandle { task_id, generation };
    let _ = BayesTaskHandle::issue_for_worker(task_id, generation);
    let _ = result.task;
}
"#,
    )
    .expect("the fixture must parse");
    assert!(fixture.iter().any(|finding| finding.kind == "broad-import"));
    assert!(fixture.iter().any(|finding| finding.kind == "construction"));
    assert!(
        fixture
            .iter()
            .any(|finding| finding.kind == "authority-call")
    );
    assert!(fixture.iter().any(|finding| finding.kind == "field-access"));

    let worker_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/worker.rs"),
    )
    .expect("Bayes worker contract source must be readable");
    let surface = bayes_worker_surface_violations(&worker_source)
        .expect("Bayes worker contract source must parse");
    assert!(
        surface.is_empty(),
        "Bayes worker authority/result fields or constructors became public: {surface:#?}"
    );
}

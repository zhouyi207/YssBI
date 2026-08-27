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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorkerFunction {
    owner: &'static str,
    method: &'static str,
}

const AUTHORITY_FUNCTIONS: &[WorkerFunction] = &[
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "issue_for_worker",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "mint_for_worker",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "validated_worker_result",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "from_worker",
    },
];
const ALLOWED_WORKER_FUNCTIONS: &[WorkerFunction] = &[
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "try_new",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "task_id",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "model",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "inputs",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "issue_for_worker",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "task_id",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "generation",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "mint_for_worker",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "artifact_id",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "handle",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "media_type",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "bytes",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "summaries",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "diagnostics",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "validated_worker_result",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "inference",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "artifacts",
    },
];
const PUBLIC_ASSOCIATED_FUNCTIONS: &[WorkerFunction] = &[WorkerFunction {
    owner: "ValidatedBayesTask",
    method: "try_new",
}];
const SEALED_AUTHORITY_TYPES: &[&str] = &[
    "BayesTaskHandle",
    "BayesArtifactHandle",
    "BayesTaskResult",
    "BayesArtifact",
    "BayesInferenceSnapshot",
];
const WORKER_SURFACE_TYPES: &[&str] = &[
    "BayesTaskId",
    "ArtifactId",
    "ValidatedBayesTask",
    "BayesTaskHandle",
    "BayesArtifactHandle",
    "BayesTaskResult",
    "BayesArtifact",
    "BayesInferenceSnapshot",
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
    "BayesInferenceSnapshot",
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
    let allow_authority =
        source_file == WORKER_FILE || JULIA_WORKER_ADAPTER_FILES.contains(&source_file);
    let mut violations = Vec::new();
    let mut visitor = BayesWorkerAuthorityVisitor {
        source_file,
        layer,
        allow_authority,
        worker_bindings: BTreeSet::new(),
        violations: &mut violations,
    };
    visitor.visit_file(&syntax);
    violations.sort();
    Ok(violations)
}

struct BayesWorkerAuthorityVisitor<'a> {
    source_file: &'a str,
    layer: RustLayer,
    allow_authority: bool,
    worker_bindings: BTreeSet<String>,
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

    fn inspect_worker_function(&mut self, path: &syn::Path, call: bool) {
        let Some((owner, method)) = path_owner_and_method(path) else {
            return;
        };
        if !WORKER_SURFACE_TYPES.contains(&owner.as_str()) {
            return;
        }
        let target = format!("{owner}::{method}");
        if authority_function(&owner, &method).is_some() {
            if !self.allow_authority {
                self.record(
                    if call {
                        "authority-call"
                    } else {
                        "authority-reference"
                    },
                    target,
                );
            }
        } else if !allowed_worker_function(&owner, &method) {
            self.record("forbidden-associated-function", target);
        }
    }

    fn record_worker_binding(&mut self, pattern: &syn::Pat, ty: &Type) {
        let syn::Pat::Ident(binding) = pattern else {
            return;
        };
        if type_owner(ty).is_some_and(|owner| WORKER_SURFACE_TYPES.contains(&owner.as_str())) {
            self.worker_bindings.insert(binding.ident.to_string());
        }
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

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let previous = self.worker_bindings.clone();
        for argument in &function.sig.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                self.record_worker_binding(&argument.pat, &argument.ty);
            }
        }
        visit::visit_item_fn(self, function);
        self.worker_bindings = previous;
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        let previous = self.worker_bindings.clone();
        for argument in &function.sig.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                self.record_worker_binding(&argument.pat, &argument.ty);
            }
        }
        visit::visit_impl_item_fn(self, function);
        self.worker_bindings = previous;
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Type(typed) = &local.pat {
            self.record_worker_binding(&typed.pat, &typed.ty);
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = call.func.as_ref() {
            self.inspect_worker_function(&path.path, true);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        self.inspect_worker_function(&path.path, false);
        visit::visit_expr_path(self, path);
    }

    fn visit_expr_struct(&mut self, construction: &'ast syn::ExprStruct) {
        if !self.allow_authority
            && let Some(ty) = construction.path.segments.last()
            && SEALED_AUTHORITY_TYPES.contains(&ty.ident.to_string().as_str())
        {
            self.record("construction", ty.ident.to_string());
        }
        visit::visit_expr_struct(self, construction);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if !self.allow_authority
            && self.layer_forbids_field_access()
            && let Expr::Path(receiver) = field.base.as_ref()
            && let Some(binding) = receiver.path.segments.last()
            && self.worker_bindings.contains(&binding.ident.to_string())
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

fn path_owner_and_method(path: &syn::Path) -> Option<(String, String)> {
    let mut segments = path.segments.iter().rev();
    let method = segments.next()?.ident.to_string();
    let owner = segments.next()?.ident.to_string();
    Some((owner, method))
}

fn type_owner(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => type_owner(&reference.elem),
        _ => None,
    }
}

fn authority_function(owner: &str, method: &str) -> Option<WorkerFunction> {
    AUTHORITY_FUNCTIONS
        .iter()
        .copied()
        .find(|function| function.owner == owner && function.method == method)
}

fn allowed_worker_function(owner: &str, method: &str) -> bool {
    ALLOWED_WORKER_FUNCTIONS
        .iter()
        .any(|function| function.owner == owner && function.method == method)
}

fn bayes_worker_surface_violations(
    source: &str,
) -> Result<Vec<BayesWorkerAuthorityViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut violations = Vec::new();
    let mut required_functions = AUTHORITY_FUNCTIONS.iter().copied().collect::<BTreeSet<_>>();

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
                    if let Some(authority) = authority_function(&owner, &name) {
                        required_functions.remove(&authority);
                        if !is_crate_visibility(&function.vis) {
                            violations.push(surface_violation(
                                "authority-visibility",
                                format!("{owner}::{name}"),
                            ));
                        }
                    }
                    if function.sig.receiver().is_none()
                        && WORKER_SURFACE_TYPES.contains(&owner.as_str())
                    {
                        let allowed_public = PUBLIC_ASSOCIATED_FUNCTIONS
                            .iter()
                            .any(|function| function.owner == owner && function.method == name);
                        if matches!(function.vis, Visibility::Public(_)) && !allowed_public {
                            violations.push(surface_violation(
                                "public-associated-function",
                                format!("{owner}::{name}"),
                            ));
                        }
                        if is_crate_visibility(&function.vis)
                            && authority_function(&owner, &name).is_none()
                        {
                            violations.push(surface_violation(
                                "forbidden-associated-function",
                                format!("{owner}::{name}"),
                            ));
                        }
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
    for missing in required_functions {
        violations.push(surface_violation(
            "missing-authority-method",
            format!("{}::{}", missing.owner, missing.method),
        ));
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

fn bayes_worker_result_neutrality_violations(source: &str) -> Result<Vec<String>, syn::Error> {
    const FORBIDDEN_TYPES: &[&str] = &[
        "InferenceResult",
        "ResultArtifactManifest",
        "ResultArtifact",
        "JuliaWorkerTaskDirectory",
        "Path",
        "PathBuf",
    ];

    let syntax = syn::parse_file(source)?;
    let mut findings = BTreeSet::new();
    for item in &syntax.items {
        match item {
            Item::Use(item_use) => {
                collect_forbidden_use_names(&item_use.tree, FORBIDDEN_TYPES, &mut findings);
            }
            Item::Struct(item_struct) if item_struct.ident == "BayesTaskResult" => {
                require_exact_fields(
                    item_struct,
                    &["artifacts", "inference"],
                    "BayesTaskResult-fields",
                    &mut findings,
                );
                collect_forbidden_field_types(item_struct, FORBIDDEN_TYPES, &mut findings);
            }
            Item::Struct(item_struct) if item_struct.ident == "BayesInferenceSnapshot" => {
                require_exact_fields(
                    item_struct,
                    &["diagnostics", "summaries", "task"],
                    "BayesInferenceSnapshot-fields",
                    &mut findings,
                );
                collect_forbidden_field_types(item_struct, FORBIDDEN_TYPES, &mut findings);
            }
            _ => {}
        }
    }
    Ok(findings.into_iter().collect())
}

fn collect_forbidden_use_names(
    tree: &UseTree,
    forbidden: &[&str],
    findings: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => collect_forbidden_use_names(&path.tree, forbidden, findings),
        UseTree::Name(name) => {
            let name = name.ident.to_string();
            if forbidden.contains(&name.as_str()) {
                findings.insert(name);
            }
        }
        UseTree::Rename(rename) => {
            let name = rename.ident.to_string();
            if forbidden.contains(&name.as_str()) {
                findings.insert(name);
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_forbidden_use_names(item, forbidden, findings);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn require_exact_fields(
    item: &syn::ItemStruct,
    expected: &[&str],
    finding: &str,
    findings: &mut BTreeSet<String>,
) {
    let actual = item
        .fields
        .iter()
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|field| (*field).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        findings.insert(finding.to_owned());
    }
}

fn collect_forbidden_field_types(
    item: &syn::ItemStruct,
    forbidden: &[&str],
    findings: &mut BTreeSet<String>,
) {
    let mut visitor = ForbiddenResultTypeVisitor {
        forbidden,
        findings,
    };
    for field in &item.fields {
        visitor.visit_type(&field.ty);
    }
}

struct ForbiddenResultTypeVisitor<'a> {
    forbidden: &'a [&'a str],
    findings: &'a mut BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ForbiddenResultTypeVisitor<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        for segment in &path.path.segments {
            let name = segment.ident.to_string();
            if self.forbidden.contains(&name.as_str()) {
                self.findings.insert(name);
            }
        }
        visit::visit_type_path(self, path);
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

struct WrongOwner;

impl WrongOwner {
    fn issue_for_worker() {}
}

fn forge(result: BayesTaskResult) {
    let _ = BayesTaskHandle { task_id, generation };
    let _ = BayesTaskHandle::issue_for_worker(task_id, generation);
    let issue = BayesTaskHandle::issue_for_worker;
    let _ = issue(task_id, generation);
    WrongOwner::issue_for_worker();
    let _ = BayesTaskHandle::from_parts(task_id, generation);
    let _ = BayesTaskHandle::forge(task_id, generation);
    let _ = result.task;
}
"#,
    )
    .expect("the fixture must parse");
    assert!(fixture.iter().any(|finding| finding.kind == "broad-import"));
    assert!(fixture.iter().any(|finding| finding.kind == "construction"));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "authority-reference"
            && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "forbidden-associated-function"
            && finding.target == "BayesTaskHandle::from_parts"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "forbidden-associated-function"
            && finding.target == "BayesTaskHandle::forge"
    }));
    assert!(
        !fixture
            .iter()
            .any(|finding| finding.target == "WrongOwner::issue_for_worker")
    );
    assert!(fixture.iter().any(|finding| finding.kind == "field-access"));

    let surface_fixture = bayes_worker_surface_violations(
        r#"
pub struct BayesTaskHandle { task_id: (), generation: () }
impl BayesTaskHandle {
    pub(crate) fn issue_for_worker() {}
    pub fn task_id(&self) {}
    pub fn generation(&self) {}
    pub fn from_parts() {}
    pub fn forge() {}
}
pub struct BayesArtifactHandle { task: (), artifact: () }
impl BayesArtifactHandle { pub(crate) fn mint_for_worker() {} }
pub struct BayesArtifact { handle: (), media_type: (), bytes: () }
impl BayesArtifact { pub(crate) fn from_worker() {} }
pub struct BayesTaskResult { inference: (), artifacts: () }
impl BayesTaskResult { pub(crate) fn validated_worker_result() {} }
"#,
    )
    .expect("the authority surface fixture must parse");
    assert!(surface_fixture.iter().any(|finding| {
        finding.kind == "public-associated-function"
            && finding.target == "BayesTaskHandle::from_parts"
    }));
    assert!(surface_fixture.iter().any(|finding| {
        finding.kind == "public-associated-function" && finding.target == "BayesTaskHandle::forge"
    }));

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

#[test]
fn bayes_model_spec_exposes_only_final_adapter_projection() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let path = workspace
        .repository_root
        .join("src-tauri/src/sci/api/bayes/model.rs");
    let source = std::fs::read_to_string(&path).expect("Bayes model source must be readable");
    let syntax = syn::parse_file(&source).expect("Bayes model source must parse");

    let model = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "BayesModelSpec" => Some(item),
            _ => None,
        })
        .expect("BayesModelSpec must exist");
    let visible_fields = model
        .fields
        .iter()
        .filter(|field| !matches!(field.vis, Visibility::Inherited))
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    assert!(
        visible_fields.is_empty(),
        "BayesModelSpec fields must stay private: {visible_fields:#?}"
    );

    let public_methods = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) if impl_owner(&item.self_ty).as_deref() == Some("BayesModelSpec") => {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            ImplItem::Fn(function) if matches!(function.vis, Visibility::Public(_)) => {
                Some(function.sig.ident.to_string())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_methods,
        BTreeSet::from([
            "data_variables".to_owned(),
            "likelihood".to_owned(),
            "parameters".to_owned(),
            "predictor".to_owned(),
            "sampler".to_owned(),
        ]),
        "BayesModelSpec public capability must stay exact"
    );

    let restricted_methods = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) if impl_owner(&item.self_ty).as_deref() == Some("BayesModelSpec") => {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            ImplItem::Fn(function) => match &function.vis {
                Visibility::Restricted(restricted) => Some((
                    function.sig.ident.to_string(),
                    restricted
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                )),
                Visibility::Public(_) | Visibility::Inherited => None,
            },
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        restricted_methods,
        BTreeMap::from([
            ("dataset".to_owned(), "crate".to_owned()),
            ("display_formula".to_owned(), "crate".to_owned()),
            ("from_validated_parts".to_owned(), "super".to_owned()),
            ("parameter_names".to_owned(), "super".to_owned()),
            ("response".to_owned(), "crate".to_owned()),
        ]),
        "BayesModelSpec internal construction and old-route projection must stay exact"
    );
}

#[test]
fn bayes_worker_result_is_neutral_and_path_free() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let worker_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/worker.rs"),
    )
    .expect("Bayes worker source must be readable");
    let actual = bayes_worker_result_neutrality_violations(&worker_source)
        .expect("Bayes worker source must parse");
    assert!(
        actual.is_empty(),
        "Bayes worker result must not own legacy result/path authority: {actual:#?}"
    );

    let contract_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/contract.rs"),
    )
    .expect("Bayes neutral contract source must be readable");
    let result_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/result.rs"),
    )
    .expect("Bayes legacy result source must be readable");
    let contract = syn::parse_file(&contract_source).expect("Bayes neutral contract must parse");
    let result = syn::parse_file(&result_source).expect("Bayes legacy result must parse");
    for symbol in [
        "ParameterSummary",
        "InferenceDiagnostics",
        "DiagnosticWarning",
        "DiagnosticMetric",
    ] {
        let in_contract = contract.items.iter().any(|item| match item {
            Item::Struct(item) => item.ident == symbol,
            Item::Enum(item) => item.ident == symbol,
            _ => false,
        });
        let in_legacy_result = result.items.iter().any(|item| match item {
            Item::Struct(item) => item.ident == symbol,
            Item::Enum(item) => item.ident == symbol,
            Item::Type(item) => item.ident == symbol,
            _ => false,
        });
        assert!(in_contract, "{symbol} must be neutral-contract owned");
        assert!(
            !in_legacy_result,
            "{symbol} must not be declared or aliased by the old result owner"
        );
    }
    for owner in [
        "InferenceResult",
        "ResultArtifactManifest",
        "ResultArtifact",
    ] {
        let item = result
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(item) if item.ident == owner => Some(item),
                _ => None,
            })
            .expect("legacy result owner must exist");
        assert!(
            item.fields
                .iter()
                .all(|field| matches!(field.vis, Visibility::Inherited)),
            "{owner} fields must stay private behind canonical getters"
        );
    }

    let fixture = bayes_worker_result_neutrality_violations(
        r#"
use crate::sci::api::bayes::result::{InferenceResult, ResultArtifactManifest};
use std::path::PathBuf;

pub struct BayesInferenceSnapshot {
    inference: InferenceResult,
    artifact_manifest: ResultArtifactManifest,
    path: PathBuf,
}

pub struct BayesTaskResult {
    inference: InferenceResult,
    artifacts: std::sync::Arc<[BayesArtifactHandle]>,
}
"#,
    )
    .expect("legacy result fixture must parse");
    assert!(fixture.iter().any(|finding| finding == "InferenceResult"));
    assert!(
        fixture
            .iter()
            .any(|finding| finding == "ResultArtifactManifest")
    );
    assert!(fixture.iter().any(|finding| finding == "PathBuf"));
}

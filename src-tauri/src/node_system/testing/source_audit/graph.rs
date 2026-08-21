use std::collections::HashSet;

use syn::visit::{self, Visit};
use syn::{Expr, Item, Meta, Type};

use super::shared::{expand_use_tree, is_test_only};

const RAW_GRAPH_DOCUMENT_MUTATIONS: [&str; 6] = [
    "create_node",
    "delete_node",
    "bind_port",
    "connect",
    "disconnect",
    "set_literal",
];

fn is_raw_graph_document_mutation(name: &syn::Ident) -> bool {
    RAW_GRAPH_DOCUMENT_MUTATIONS.contains(&name.to_string().as_str())
}

fn is_strict_cfg_test(attributes: &[syn::Attribute]) -> bool {
    let cfg_attributes = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .collect::<Vec<_>>();
    cfg_attributes.len() == 1
        && cfg_attributes[0]
            .meta
            .require_list()
            .ok()
            .and_then(|cfg| syn::parse2::<Meta>(cfg.tokens.clone()).ok())
            .is_some_and(|predicate| matches!(predicate, Meta::Path(path) if path.is_ident("test")))
}

fn is_graph_document_type(value_type: &Type) -> bool {
    matches!(
        value_type,
        Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "GraphDocument")
    )
}

fn expr_attributes(expr: &Expr) -> &[syn::Attribute] {
    match expr {
        Expr::Array(expr) => &expr.attrs,
        Expr::Assign(expr) => &expr.attrs,
        Expr::Async(expr) => &expr.attrs,
        Expr::Await(expr) => &expr.attrs,
        Expr::Binary(expr) => &expr.attrs,
        Expr::Block(expr) => &expr.attrs,
        Expr::Break(expr) => &expr.attrs,
        Expr::Call(expr) => &expr.attrs,
        Expr::Cast(expr) => &expr.attrs,
        Expr::Closure(expr) => &expr.attrs,
        Expr::Const(expr) => &expr.attrs,
        Expr::Continue(expr) => &expr.attrs,
        Expr::Field(expr) => &expr.attrs,
        Expr::ForLoop(expr) => &expr.attrs,
        Expr::Group(expr) => &expr.attrs,
        Expr::If(expr) => &expr.attrs,
        Expr::Index(expr) => &expr.attrs,
        Expr::Infer(expr) => &expr.attrs,
        Expr::Let(expr) => &expr.attrs,
        Expr::Lit(expr) => &expr.attrs,
        Expr::Loop(expr) => &expr.attrs,
        Expr::Macro(expr) => &expr.attrs,
        Expr::Match(expr) => &expr.attrs,
        Expr::MethodCall(expr) => &expr.attrs,
        Expr::Paren(expr) => &expr.attrs,
        Expr::Path(expr) => &expr.attrs,
        Expr::Range(expr) => &expr.attrs,
        Expr::RawAddr(expr) => &expr.attrs,
        Expr::Reference(expr) => &expr.attrs,
        Expr::Repeat(expr) => &expr.attrs,
        Expr::Return(expr) => &expr.attrs,
        Expr::Struct(expr) => &expr.attrs,
        Expr::Try(expr) => &expr.attrs,
        Expr::TryBlock(expr) => &expr.attrs,
        Expr::Tuple(expr) => &expr.attrs,
        Expr::Unary(expr) => &expr.attrs,
        Expr::Unsafe(expr) => &expr.attrs,
        Expr::While(expr) => &expr.attrs,
        Expr::Yield(expr) => &expr.attrs,
        _ => &[],
    }
}

struct RawGraphDocumentMutationVisitor {
    violations: Vec<String>,
    production_methods: HashSet<String>,
    strict_test_methods: HashSet<String>,
    test_only_scope: bool,
}

impl RawGraphDocumentMutationVisitor {
    fn report(&mut self, kind: &str, name: &syn::Ident) {
        self.violations.push(format!(
            "production raw GraphDocument mutation {kind}:{}",
            name
        ));
    }

    fn inspect_graph_document_impl(&mut self, node: &syn::ItemImpl) {
        if !is_graph_document_type(&node.self_ty) {
            return;
        }
        let impl_is_test_only = is_test_only(&node.attrs);
        let strict_test_impl = !self.test_only_scope && is_strict_cfg_test(&node.attrs);
        for item in &node.items {
            let syn::ImplItem::Fn(method) = item else {
                continue;
            };
            if !is_raw_graph_document_mutation(&method.sig.ident) {
                continue;
            }
            let name = method.sig.ident.to_string();
            let method_is_test_only = is_test_only(&method.attrs);
            if !self.test_only_scope && !impl_is_test_only && !method_is_test_only {
                self.production_methods.insert(name.clone());
                self.violations.push(format!(
                    "production GraphDocument impl exposes raw mutation:{name}"
                ));
            } else if strict_test_impl
                && !method.attrs.iter().any(|attr| attr.path().is_ident("cfg"))
            {
                self.strict_test_methods.insert(name.clone());
                let crate_visible = matches!(
                    &method.vis,
                    syn::Visibility::Restricted(restricted)
                        if restricted.path.is_ident("crate")
                );
                if !crate_visible {
                    self.violations.push(format!(
                        "test-only GraphDocument mutation must be pub(crate):{name}"
                    ));
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for RawGraphDocumentMutationVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_item_mod(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        self.inspect_graph_document_impl(node);
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_item_impl(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_item_fn(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_impl_item_fn(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_trait_item_fn(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(expr_attributes(node));
        visit::visit_expr(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        let parent_scope = self.test_only_scope;
        self.test_only_scope |= is_test_only(&node.attrs);
        visit::visit_local(self, node);
        self.test_only_scope = parent_scope;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if !self.test_only_scope
            && !is_test_only(&node.attrs)
            && is_raw_graph_document_mutation(&node.method)
        {
            self.report("method call", &node.method);
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if !self.test_only_scope
            && !is_test_only(&node.attrs)
            && let Some(name) = node.path.segments.last().map(|segment| &segment.ident)
            && is_raw_graph_document_mutation(name)
            && (node.qself.is_some() || node.path.segments.len() > 1)
        {
            self.report("UFCS or alias reference", name);
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        if self.test_only_scope || is_test_only(&node.attrs) {
            return;
        }
        let mut paths = Vec::new();
        expand_use_tree(&node.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            if let Some(name) = path
                .iter()
                .find(|segment| RAW_GRAPH_DOCUMENT_MUTATIONS.contains(&segment.as_str()))
            {
                self.violations.push(format!(
                    "production raw GraphDocument mutation import alias:{name}"
                ));
            }
        }
        visit::visit_item_use(self, node);
    }
}

pub(super) fn audit_raw_graph_document_mutations(transaction_source: &str) -> Vec<String> {
    let transaction = syn::parse_file(transaction_source).unwrap();
    let mut visitor = RawGraphDocumentMutationVisitor {
        violations: Vec::new(),
        production_methods: HashSet::new(),
        strict_test_methods: HashSet::new(),
        test_only_scope: false,
    };
    visitor.visit_file(&transaction);

    for name in RAW_GRAPH_DOCUMENT_MUTATIONS {
        if !visitor.production_methods.contains(name) && !visitor.strict_test_methods.contains(name)
        {
            visitor.violations.push(format!(
                "GraphDocument test mutation is missing strict cfg(test) impl:{name}"
            ));
        }
    }

    visitor.violations.sort();
    visitor.violations.dedup();
    visitor.violations
}

pub(super) fn audit_production_graph_write_surface(
    document_source: &str,
    mutation_source: &str,
    project_state_source: &str,
) -> Vec<String> {
    let document_module = syn::parse_file(document_source).unwrap();
    let mutation_module = syn::parse_file(mutation_source).unwrap();
    let project_state = syn::parse_file(project_state_source).unwrap();
    let mut violations = Vec::new();

    let mut public_document_exports = HashSet::new();
    let mut has_public_glob = false;
    for item in &document_module.items {
        let Item::Use(item_use) = item else {
            continue;
        };
        if !matches!(item_use.vis, syn::Visibility::Public(_)) || is_test_only(&item_use.attrs) {
            continue;
        }
        let mut paths = Vec::new();
        expand_use_tree(&item_use.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            has_public_glob |= path.last().is_some_and(|segment| segment == "*");
            if let Some(export) = path.last() {
                public_document_exports.insert(export.clone());
            }
        }
    }

    if has_public_glob {
        violations.push(
            "production public glob re-export from mutation or another module is forbidden"
                .to_owned(),
        );
    }
    for raw_export in ["GraphMutation", "RevisionedGraphStore", "apply_mutation"] {
        if public_document_exports.contains(raw_export) {
            violations.push(format!(
                "raw graph write symbol {raw_export} must not be publicly re-exported in production"
            ));
        }
    }
    if !public_document_exports.contains("GraphDocumentPatch") {
        violations.push(
            "GraphDocumentPatch must remain publicly available as committed delta/History data"
                .to_owned(),
        );
    }

    if mutation_module.items.iter().any(|item| {
        matches!(
            item,
            Item::Fn(function)
                if function.sig.ident == "apply_mutation"
                    && matches!(function.vis, syn::Visibility::Public(_))
                    && !is_test_only(&function.attrs)
        )
    }) {
        violations
            .push("no public production free function named apply_mutation may remain".to_owned());
    }

    let mut public_project_state_methods = HashSet::new();
    for item in &project_state.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        let Type::Path(self_type) = item_impl.self_ty.as_ref() else {
            continue;
        };
        if !self_type
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "ProjectState")
        {
            continue;
        }
        for impl_item in &item_impl.items {
            let syn::ImplItem::Fn(method) = impl_item else {
                continue;
            };
            if matches!(method.vis, syn::Visibility::Public(_)) && !is_test_only(&method.attrs) {
                public_project_state_methods.insert(method.sig.ident.to_string());
            }
        }
    }

    for raw_method in ["apply_graph_mutation", "apply_graph_patch"] {
        if public_project_state_methods.contains(raw_method) {
            violations.push(format!(
                "ProjectState::{raw_method} must not be public in production"
            ));
        }
    }
    if !public_project_state_methods.contains("apply_editor_graph_mutation") {
        violations.push("ProjectState::apply_editor_graph_mutation must remain public".to_owned());
    }

    violations
}

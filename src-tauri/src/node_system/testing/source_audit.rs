use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, Item, Lit, Macro, Member, Meta, Pat, ReturnType, Token, Type, UseTree};

const AUDIT_SOURCE: &str = "node_system/testing/source_audit.rs";
const REGISTRY_AUTHORITY: &str = "node_system/registry/model.rs";
const ASSEMBLY_FORBIDDEN_SYMBOLS: &[&str] = &[
    "ASSEMBLY_PROTOCOL_ERROR",
    "record_protocol_error",
    "run_assembly",
    "AssemblySemanticId",
    "from_unvalidated_assembly",
];
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
    } else if contains_pair("graph", "core") {
        Some(("old graph core path", "graph::core"))
    } else if contains_pair("graph", "infer") {
        Some(("old graph inference path", "graph::infer"))
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

fn static_string_expression(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.value()),
        Expr::Macro(expression) if expression.mac.path.is_ident("concat") => {
            let arguments = macro_arguments(&expression.mac)?;
            let mut combined = String::new();
            for argument in &arguments {
                combined.push_str(&static_string_expression(argument)?);
            }
            Some(combined)
        }
        Expr::Paren(expression) => static_string_expression(&expression.expr),
        Expr::Group(expression) => static_string_expression(&expression.expr),
        _ => None,
    }
}

fn static_include_path(mac: &Macro) -> Option<String> {
    let expression = syn::parse2::<Expr>(mac.tokens.clone()).ok()?;
    static_string_expression(&expression)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MacroToken {
    Ident(String),
    Punct(char),
}

fn flatten_macro_tokens(tokens: TokenStream, flattened: &mut Vec<MacroToken>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => flatten_macro_tokens(group.stream(), flattened),
            TokenTree::Ident(ident) => flattened.push(MacroToken::Ident(ident.to_string())),
            TokenTree::Punct(punct) => flattened.push(MacroToken::Punct(punct.as_char())),
            TokenTree::Literal(_) => {}
        }
    }
}

fn collect_include_fragments(tokens: TokenStream, fragments: &mut Vec<String>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => collect_include_fragments(group.stream(), fragments),
            TokenTree::Ident(ident) => fragments.push(ident.to_string()),
            TokenTree::Literal(literal) => {
                if let Ok(value) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                    fragments.extend(
                        value
                            .value()
                            .replace('\\', "/")
                            .split('/')
                            .filter(|fragment| !fragment.is_empty())
                            .map(str::to_owned),
                    );
                }
            }
            TokenTree::Punct(_) => {}
        }
    }
}

fn include_tokens_contain_legacy_fragments(tokens: TokenStream) -> bool {
    if is_legacy_module_path(&tokens.to_string()) {
        return true;
    }
    let mut fragments = Vec::new();
    collect_include_fragments(tokens, &mut fragments);
    fragments.windows(2).any(|pair| {
        matches!(
            (pair[0].as_str(), pair[1].as_str()),
            ("graph", "register")
                | ("graph", "core")
                | ("graph", "infer")
                | ("execution", "context")
                | ("execution", "engine")
        )
    })
}

fn macro_path(flattened: &[MacroToken], first: &str, second: &str) -> bool {
    flattened.windows(4).any(|window| {
        matches!(
            window,
            [
                MacroToken::Ident(left),
                MacroToken::Punct(':'),
                MacroToken::Punct(':'),
                MacroToken::Ident(right),
            ] if left == first && right == second
        )
    })
}

fn macro_defines_node_registry(flattened: &[MacroToken]) -> bool {
    flattened.windows(2).any(|window| {
        matches!(
            window,
            [MacroToken::Ident(kind), MacroToken::Ident(name)]
                if matches!(kind.as_str(), "struct" | "enum" | "union" | "trait" | "type")
                    && name == "NodeRegistry"
        )
    })
}

fn macro_has_ident(flattened: &[MacroToken], expected: &str) -> bool {
    flattened
        .iter()
        .any(|token| matches!(token, MacroToken::Ident(ident) if ident == expected))
}

fn expression_builds_category_identity(expr: &Expr) -> bool {
    match expr {
        Expr::Macro(expression) if expression.mac.path.is_ident("format") => {
            format_builds_category_identity(&expression.mac)
        }
        Expr::Macro(expression) if expression.mac.path.is_ident("concat") => {
            concat_builds_category_identity(&expression.mac)
        }
        Expr::MethodCall(call) if call.method == "join" => {
            expr_mentions(&call.receiver, &["category", "categories"])
                && matches!(call.args.first(), Some(Expr::Lit(ExprLit { lit: Lit::Str(value), .. })) if value.value() == ":")
        }
        Expr::Call(call) => {
            let category = call
                .args
                .iter()
                .any(|argument| expr_mentions(argument, &["category", "categories"]));
            let label = call
                .args
                .iter()
                .any(|argument| expr_mentions(argument, &["name", "title"]));
            (category && label) || call.args.iter().any(expression_builds_category_identity)
        }
        Expr::MethodCall(call) => {
            expression_builds_category_identity(&call.receiver)
                || call.args.iter().any(expression_builds_category_identity)
        }
        Expr::Binary(binary) => {
            expression_builds_category_identity(&binary.left)
                || expression_builds_category_identity(&binary.right)
                || (expr_mentions(expr, &["category", "categories"])
                    && expr_mentions(expr, &["name", "title"]))
        }
        Expr::Paren(expression) => expression_builds_category_identity(&expression.expr),
        Expr::Group(expression) => expression_builds_category_identity(&expression.expr),
        Expr::Reference(expression) => expression_builds_category_identity(&expression.expr),
        _ => false,
    }
}

fn pattern_ident(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Type(typed) => pattern_ident(&typed.pat),
        Pat::Paren(parenthesized) => pattern_ident(&parenthesized.pat),
        _ => None,
    }
}

fn pattern_is_identity_sink(pattern: &Pat) -> bool {
    match pattern {
        Pat::Ident(ident) => {
            matches!(
                ident.ident.to_string().as_str(),
                "node_type" | "node_type_id"
            )
        }
        Pat::Type(typed) => pattern_is_identity_sink(&typed.pat),
        Pat::Paren(parenthesized) => pattern_is_identity_sink(&parenthesized.pat),
        _ => false,
    }
}

fn expression_is_identity_sink(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) => path.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "node_type" | "node_type_id"
            )
        }),
        Expr::Field(field) => matches!(
            &field.member,
            Member::Named(name)
                if matches!(name.to_string().as_str(), "node_type" | "node_type_id")
        ),
        Expr::Paren(expression) => expression_is_identity_sink(&expression.expr),
        _ => false,
    }
}

fn type_is_node_type_id(value_type: &Type) -> bool {
    matches!(
        value_type,
        Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "NodeTypeId")
    )
}

struct SourceVisitor<'a> {
    relative: &'a str,
    source: &'a str,
    offenders: &'a mut Vec<String>,
    module_path: Vec<String>,
    identity_returns: Vec<bool>,
    bindings: Vec<HashMap<String, Expr>>,
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

    fn binding(&self, name: &str) -> Option<&Expr> {
        self.bindings.iter().rev().find_map(|scope| scope.get(name))
    }

    fn resolved_mentions(
        &self,
        expr: &Expr,
        names: &[&str],
        visiting: &mut HashSet<String>,
    ) -> bool {
        if expr_mentions(expr, names) {
            return true;
        }
        match expr {
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let found = self
                    .binding(&name)
                    .is_some_and(|bound| self.resolved_mentions(bound, names, visiting));
                visiting.remove(&name);
                found
            }
            Expr::Binary(binary) => {
                self.resolved_mentions(&binary.left, names, visiting)
                    || self.resolved_mentions(&binary.right, names, visiting)
            }
            Expr::MethodCall(call) => {
                self.resolved_mentions(&call.receiver, names, visiting)
                    || call
                        .args
                        .iter()
                        .any(|argument| self.resolved_mentions(argument, names, visiting))
            }
            Expr::Call(call) => {
                self.resolved_mentions(&call.func, names, visiting)
                    || call
                        .args
                        .iter()
                        .any(|argument| self.resolved_mentions(argument, names, visiting))
            }
            Expr::Reference(reference) => self.resolved_mentions(&reference.expr, names, visiting),
            Expr::Paren(paren) => self.resolved_mentions(&paren.expr, names, visiting),
            Expr::Group(group) => self.resolved_mentions(&group.expr, names, visiting),
            _ => false,
        }
    }

    fn builds_category_identity(&self, expr: &Expr) -> bool {
        if expression_builds_category_identity(expr) {
            return true;
        }
        if let Expr::Path(path) = expr {
            if path.path.segments.len() == 1 {
                let name = path.path.segments[0].ident.to_string();
                let mut visiting = HashSet::from([name.clone()]);
                return self.binding(&name).is_some_and(|bound| {
                    self.builds_category_identity_resolved(bound, &mut visiting)
                });
            }
        }
        self.builds_category_identity_resolved(expr, &mut HashSet::new())
    }

    fn builds_category_identity_resolved(
        &self,
        expr: &Expr,
        visiting: &mut HashSet<String>,
    ) -> bool {
        if expression_builds_category_identity(expr) {
            return true;
        }
        if let Expr::Path(path) = expr {
            if path.path.segments.len() == 1 {
                let name = path.path.segments[0].ident.to_string();
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let found = self
                    .binding(&name)
                    .is_some_and(|bound| self.builds_category_identity_resolved(bound, visiting));
                visiting.remove(&name);
                return found;
            }
        }
        let category =
            self.resolved_mentions(expr, &["category", "categories"], &mut visiting.clone());
        let label = self.resolved_mentions(expr, &["name", "title"], &mut visiting.clone());
        if category && label && matches!(expr, Expr::Binary(_) | Expr::Call(_) | Expr::Macro(_)) {
            return true;
        }
        match expr {
            Expr::MethodCall(call) => {
                self.builds_category_identity_resolved(&call.receiver, visiting)
                    || call
                        .args
                        .iter()
                        .any(|argument| self.builds_category_identity_resolved(argument, visiting))
            }
            Expr::Binary(binary) => {
                self.builds_category_identity_resolved(&binary.left, visiting)
                    || self.builds_category_identity_resolved(&binary.right, visiting)
            }
            Expr::Call(call) => call
                .args
                .iter()
                .any(|argument| self.builds_category_identity_resolved(argument, visiting)),
            Expr::Reference(reference) => {
                self.builds_category_identity_resolved(&reference.expr, visiting)
            }
            Expr::Paren(paren) => self.builds_category_identity_resolved(&paren.expr, visiting),
            Expr::Group(group) => self.builds_category_identity_resolved(&group.expr, visiting),
            _ => false,
        }
    }

    fn inspect_macro(&mut self, mac: &Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if name.as_deref() == Some("include") {
            let static_path = static_include_path(mac);
            let legacy = static_path.as_deref().is_some_and(is_legacy_module_path)
                || static_path.is_none()
                    && include_tokens_contain_legacy_fragments(mac.tokens.clone());
            if legacy {
                self.report("legacy module include", "include!");
            }
        }

        let mut flattened = Vec::new();
        flatten_macro_tokens(mac.tokens.clone(), &mut flattened);
        if macro_defines_node_registry(&flattened) {
            self.report("macro NodeRegistry definition", "NodeRegistry");
        }
        for (ident, label) in [
            ("GraphInstance", "legacy GraphInstance"),
            ("NodeDefinition", "legacy node definition"),
            ("NodeExecutionContext", "old execution context type"),
            ("ExecutionStack", "old execution engine type"),
            ("ExecutionFrame", "old execution engine type"),
            ("Executor", "old execution engine type"),
            ("reconcile_node_pins", "dynamic pin reconciliation"),
            ("resolve_dynamic_pins", "dynamic pin resolution"),
            ("resolve_all_dynamic_pins", "dynamic pin resolution"),
            ("sync_static_pin_definitions", "static pin reconciliation"),
        ] {
            if macro_has_ident(&flattened, ident) {
                self.report(label, ident);
            }
        }
        for (first, second, label, token) in [
            (
                "graph",
                "register",
                "graph Registry path",
                "graph::register",
            ),
            ("graph", "core", "old graph core path", "graph::core"),
            ("graph", "infer", "old graph inference path", "graph::infer"),
            (
                "execution",
                "context",
                "old execution context path",
                "execution::context",
            ),
            (
                "execution",
                "engine",
                "old execution engine path",
                "execution::engine",
            ),
        ] {
            if macro_path(&flattened, first, second) {
                self.report(label, token);
            }
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
            Item::Union(item) if item.ident == "NodeRegistry" => {
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

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.module_path.push(node.ident.to_string());
        if let Some((label, token)) = path_label(&self.module_path) {
            self.report(label, token);
        }
        visit::visit_item_mod(self, node);
        self.module_path.pop();
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let named_sink = matches!(
            node.sig.ident.to_string().as_str(),
            "node_type" | "node_type_id"
        );
        let typed_sink = matches!(
            &node.sig.output,
            ReturnType::Type(_, value_type) if type_is_node_type_id(value_type)
        );
        let identity_return = named_sink || typed_sink;
        self.identity_returns.push(identity_return);
        if identity_return {
            if let Some(syn::Stmt::Expr(expression, None)) = node.block.stmts.last() {
                if self.builds_category_identity(expression) {
                    self.report("category/name identity", "node type return");
                }
            }
        }
        visit::visit_item_fn(self, node);
        self.identity_returns.pop();
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
            if path
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "NodeTypeId")
                && node
                    .args
                    .iter()
                    .any(|argument| self.builds_category_identity(argument))
            {
                self.report("category/name identity", "NodeTypeId");
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "placeholder" {
            self.report("placeholder node definition", "placeholder");
        }
        if node.method == "insert"
            && expr_mentions(&node.receiver, &["registry"])
            && node
                .args
                .first()
                .is_some_and(|argument| self.builds_category_identity(argument))
        {
            self.report("category/name identity", "registry.insert");
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if pattern_is_identity_sink(&node.pat)
            && node
                .init
                .as_ref()
                .is_some_and(|init| self.builds_category_identity(&init.expr))
        {
            self.report("category/name identity", "node_type_id assignment");
        }
        visit::visit_local(self, node);
        if let (Some(name), Some(init), Some(scope)) = (
            pattern_ident(&node.pat),
            node.init.as_ref(),
            self.bindings.last_mut(),
        ) {
            scope.insert(name, (*init.expr).clone());
        }
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if expression_is_identity_sink(&node.left) && self.builds_category_identity(&node.right) {
            self.report("category/name identity", "node_type assignment");
        }
        visit::visit_expr_assign(self, node);
    }

    fn visit_field_value(&mut self, node: &'ast syn::FieldValue) {
        if matches!(
            &node.member,
            Member::Named(name)
                if matches!(name.to_string().as_str(), "node_type" | "node_type_id")
        ) && self.builds_category_identity(&node.expr)
        {
            self.report("category/name identity", "node_type field assignment");
        }
        visit::visit_field_value(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        if expr_mentions(&node.expr, &["registry"]) && self.builds_category_identity(&node.index) {
            self.report("category/name identity", "Registry key");
        }
        visit::visit_expr_index(self, node);
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        if self.identity_returns.last().copied().unwrap_or(false)
            && node
                .expr
                .as_ref()
                .is_some_and(|expr| self.builds_category_identity(expr))
        {
            self.report("category/name identity", "node type return");
        }
        visit::visit_expr_return(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if matches!(&node.member, Member::Named(name) if name == "name")
            && expr_mentions(&node.base, &["definition", "pin_definition"])
        {
            self.report("display-name pin matching", "pin.definition.name");
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.bindings.push(HashMap::new());
        visit::visit_block(self, node);
        self.bindings.pop();
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
            module_path: Vec::new(),
            identity_returns: Vec::new(),
            bindings: Vec::new(),
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

fn is_test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg")
                && attribute
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string().contains("test")))
    })
}

fn is_complete_assembly_path(relative: &str) -> bool {
    (relative.starts_with("node_system/catalog/")
        && !relative.ends_with("tests.rs")
        && !relative.contains("/tests/"))
        || matches!(
            relative,
            "node_system/registry/mod.rs"
                | "node_system/registry/validation.rs"
                | "node_system/registry/fingerprint.rs"
        )
}

fn is_assembly_function(name: &str) -> bool {
    name.starts_with("assemble_")
        || name.starts_with("build_provider_fragment")
        || name.starts_with("registered_node")
        || name.ends_with("_protocol")
        || matches!(
            name,
            "assembled_interface"
                | "assembled_parameters"
                | "build_builtin_node_system"
                | "validate_builtin_bundle"
                | "register"
                | "finish"
                | "protocol"
                | "port"
                | "data_port"
                | "control_port"
                | "effect_port"
                | "parameter"
                | "semantic"
                | "sid"
                | "iid"
                | "i18n"
                | "category"
                | "try_new"
                | "try_with_filesystem"
                | "initialize_project_state"
                | "initialize_project_state_before_manage"
        )
}

fn returns_result(output: &ReturnType) -> bool {
    matches!(
        output,
        ReturnType::Type(_, return_type)
            if matches!(return_type.as_ref(), Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Result"))
    )
}

fn module_path_for_file(relative: &str) -> Vec<String> {
    let mut segments = relative
        .trim_end_matches(".rs")
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if segments.last().is_some_and(|segment| segment == "mod") {
        segments.pop();
    } else if segments
        .last()
        .is_some_and(|segment| matches!(segment.as_str(), "lib" | "main"))
    {
        segments.pop();
    }
    segments
}

fn canonical_path(segments: &[String]) -> String {
    segments.join("::")
}

fn unambiguous_contract(contracts: Option<&Vec<bool>>) -> Option<bool> {
    let contracts = contracts?;
    let first = *contracts.first()?;
    contracts
        .iter()
        .all(|contract| *contract == first)
        .then_some(first)
}

#[derive(Default)]
struct AssemblyFunctionIndex {
    functions: HashMap<String, bool>,
    associated_functions: HashMap<(String, String), Vec<bool>>,
    methods: HashMap<String, Vec<bool>>,
    imports: HashMap<String, HashMap<String, Vec<String>>>,
}

fn canonicalize_use_path(module: &[String], raw: &[String]) -> Vec<String> {
    let mut resolved = module.to_vec();
    let mut offset = 0;
    match raw.first().map(String::as_str) {
        Some("crate") => {
            resolved.clear();
            offset = 1;
        }
        Some("self") => offset = 1,
        Some("super") => {
            while raw.get(offset).is_some_and(|segment| segment == "super") {
                resolved.pop();
                offset += 1;
            }
        }
        _ => resolved.clear(),
    }
    resolved.extend(raw[offset..].iter().cloned());
    resolved
}

fn collect_use_bindings(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    bindings: &mut Vec<(String, Vec<String>)>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_bindings(&path.tree, prefix, bindings);
            prefix.pop();
        }
        UseTree::Name(name) if name.ident == "self" => {
            if let Some(alias) = prefix.last() {
                bindings.push((alias.clone(), prefix.clone()));
            }
        }
        UseTree::Name(name) => {
            let mut target = prefix.clone();
            target.push(name.ident.to_string());
            bindings.push((name.ident.to_string(), target));
        }
        UseTree::Rename(rename) => {
            let mut target = prefix.clone();
            target.push(rename.ident.to_string());
            bindings.push((rename.rename.to_string(), target));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_bindings(item, prefix, bindings);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn index_associated_function(
    index: &mut AssemblyFunctionIndex,
    module: &[String],
    type_name: &str,
    function_name: &str,
    fallible: bool,
) {
    let mut path = module.to_vec();
    path.extend([type_name.to_owned(), function_name.to_owned()]);
    index.functions.insert(canonical_path(&path), fallible);
    index
        .associated_functions
        .entry((type_name.to_owned(), function_name.to_owned()))
        .or_default()
        .push(fallible);
    index
        .methods
        .entry(function_name.to_owned())
        .or_default()
        .push(fallible);
}

fn semantic_id_macro_type(item: &syn::ItemMacro) -> Option<String> {
    if !item.mac.path.is_ident("semantic_id") {
        return None;
    }
    item.mac
        .tokens
        .clone()
        .into_iter()
        .find_map(|token| match token {
            TokenTree::Ident(ident) => Some(ident.to_string()),
            _ => None,
        })
}

fn index_assembly_items(
    items: &[Item],
    module: &mut Vec<String>,
    index: &mut AssemblyFunctionIndex,
) {
    for item in items {
        match item {
            Item::Fn(function) if !is_test_only(&function.attrs) => {
                let mut path = module.clone();
                path.push(function.sig.ident.to_string());
                index
                    .functions
                    .insert(canonical_path(&path), returns_result(&function.sig.output));
            }
            Item::Impl(item_impl) if !is_test_only(&item_impl.attrs) => {
                let Type::Path(self_type) = item_impl.self_ty.as_ref() else {
                    continue;
                };
                let Some(type_name) = self_type.path.segments.last() else {
                    continue;
                };
                for item in &item_impl.items {
                    if let syn::ImplItem::Fn(function) = item {
                        if is_test_only(&function.attrs) {
                            continue;
                        }
                        index_associated_function(
                            index,
                            module,
                            &type_name.ident.to_string(),
                            &function.sig.ident.to_string(),
                            returns_result(&function.sig.output),
                        );
                    }
                }
            }
            Item::Macro(item_macro) => {
                if let Some(type_name) = semantic_id_macro_type(item_macro) {
                    index_associated_function(index, module, &type_name, "new", true);
                }
            }
            Item::Use(item_use) if !is_test_only(&item_use.attrs) => {
                let mut bindings = Vec::new();
                collect_use_bindings(&item_use.tree, &mut Vec::new(), &mut bindings);
                let module_imports = index.imports.entry(canonical_path(module)).or_default();
                for (alias, target) in bindings {
                    module_imports.insert(alias, canonicalize_use_path(module, &target));
                }
            }
            Item::Mod(item_mod) if !is_test_only(&item_mod.attrs) => {
                if let Some((_, items)) = &item_mod.content {
                    module.push(item_mod.ident.to_string());
                    index_assembly_items(items, module, index);
                    module.pop();
                }
            }
            _ => {}
        }
    }
}

fn build_assembly_function_index(sources: &[(String, String)]) -> AssemblyFunctionIndex {
    let mut index = AssemblyFunctionIndex::default();
    for (relative, source) in sources {
        if let Ok(syntax) = syn::parse_file(source) {
            index_assembly_items(
                &syntax.items,
                &mut module_path_for_file(relative),
                &mut index,
            );
        }
    }
    index
}

struct AssemblyEscapeVisitor<'a> {
    relative: &'a str,
    offenders: &'a mut Vec<String>,
    assembly_depth: usize,
    audit_all_functions: bool,
    module_path: Vec<String>,
    local_callables: Vec<HashMap<String, bool>>,
    function_index: &'a AssemblyFunctionIndex,
}

impl AssemblyEscapeVisitor<'_> {
    fn in_assembly(&self) -> bool {
        self.assembly_depth > 0
    }

    fn report(&mut self, label: &str, token: &str) {
        record(self.offenders, self.relative, 1, label, token);
    }

    fn resolve_call_path(&self, path: &syn::Path) -> Option<bool> {
        let raw = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let first = raw.first()?;
        let candidates = if raw.len() == 1 {
            if let Some(fallible) = self
                .local_callables
                .iter()
                .rev()
                .find_map(|scope| scope.get(first))
            {
                return Some(*fallible);
            }
            let mut local = self.module_path.clone();
            local.push(first.clone());
            let local_key = canonical_path(&local);
            if let Some(fallible) = self.function_index.functions.get(&local_key) {
                return Some(*fallible);
            }
            self.function_index
                .imports
                .get(&canonical_path(&self.module_path))
                .and_then(|imports| imports.get(first))
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        } else if matches!(first.as_str(), "crate" | "self" | "super") {
            vec![canonicalize_use_path(&self.module_path, &raw)]
        } else if let Some(target) = self
            .function_index
            .imports
            .get(&canonical_path(&self.module_path))
            .and_then(|imports| imports.get(first))
        {
            let mut target = target.clone();
            target.extend(raw.iter().skip(1).cloned());
            vec![target]
        } else {
            let mut relative = self.module_path.clone();
            relative.extend(raw.iter().cloned());
            vec![relative, raw.clone()]
        };
        candidates
            .into_iter()
            .find_map(|candidate| {
                self.function_index
                    .functions
                    .get(&canonical_path(&candidate))
                    .copied()
            })
            .or_else(|| {
                (raw.len() >= 2).then(|| {
                    let key = (raw[raw.len() - 2].clone(), raw[raw.len() - 1].clone());
                    unambiguous_contract(self.function_index.associated_functions.get(&key))
                })?
            })
    }

    fn callable_binding_contract(&self, expression: &Expr) -> Option<bool> {
        match expression {
            Expr::Closure(closure) => Some(returns_result(&closure.output)),
            Expr::Path(path) => Some(self.resolve_call_path(&path.path).unwrap_or(false)),
            Expr::Paren(paren) => self.callable_binding_contract(&paren.expr),
            Expr::Group(group) => self.callable_binding_contract(&group.expr),
            Expr::Reference(reference) => self.callable_binding_contract(&reference.expr),
            _ => None,
        }
    }

    fn is_resolved_fallible_call(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Call(call) => match call.func.as_ref() {
                Expr::Path(path) if path.path.is_ident("drop") => call
                    .args
                    .iter()
                    .any(|argument| self.is_resolved_fallible_call(argument)),
                Expr::Path(path) => self.resolve_call_path(&path.path) == Some(true),
                _ => false,
            },
            Expr::MethodCall(call) => {
                unambiguous_contract(self.function_index.methods.get(&call.method.to_string()))
                    == Some(true)
            }
            Expr::Paren(paren) => self.is_resolved_fallible_call(&paren.expr),
            Expr::Group(group) => self.is_resolved_fallible_call(&group.expr),
            Expr::Reference(reference) => self.is_resolved_fallible_call(&reference.expr),
            _ => false,
        }
    }
}

impl Visit<'_> for AssemblyEscapeVisitor<'_> {
    fn visit_block(&mut self, node: &syn::Block) {
        let local_functions = node
            .stmts
            .iter()
            .filter_map(|statement| match statement {
                syn::Stmt::Item(Item::Fn(function)) if !is_test_only(&function.attrs) => Some((
                    function.sig.ident.to_string(),
                    returns_result(&function.sig.output),
                )),
                _ => None,
            })
            .collect();
        self.local_callables.push(local_functions);
        visit::visit_block(self, node);
        self.local_callables.pop();
    }

    fn visit_item_mod(&mut self, node: &syn::ItemMod) {
        if is_test_only(&node.attrs) {
            return;
        }
        self.module_path.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.module_path.pop();
    }

    fn visit_item_fn(&mut self, node: &syn::ItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let assembly =
            self.audit_all_functions || is_assembly_function(&node.sig.ident.to_string());
        self.assembly_depth += usize::from(assembly);
        visit::visit_item_fn(self, node);
        self.assembly_depth -= usize::from(assembly);
    }

    fn visit_impl_item_fn(&mut self, node: &syn::ImplItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let assembly =
            self.audit_all_functions || is_assembly_function(&node.sig.ident.to_string());
        self.assembly_depth += usize::from(assembly);
        visit::visit_impl_item_fn(self, node);
        self.assembly_depth -= usize::from(assembly);
    }

    fn visit_ident(&mut self, ident: &syn::Ident) {
        let value = ident.to_string();
        if ASSEMBLY_FORBIDDEN_SYMBOLS.contains(&value.as_str()) {
            self.report("built-in assembly escape hatch", &value);
        }
        if self.in_assembly() && value == "fallback" {
            self.report("fallback assembly value", &value);
        }
    }

    fn visit_expr_struct(&mut self, node: &syn::ExprStruct) {
        if self.in_assembly()
            && node.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "NodeInterfaceProtocol" | "ParameterSchema"
                )
            })
        {
            self.report("fallback assembly value", "raw protocol/schema literal");
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &syn::ExprMethodCall) {
        let method = node.method.to_string();
        if self.in_assembly() && (method.starts_with("unwrap") || method == "expect") {
            self.report("assembly panic shortcut", &method);
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_macro(&mut self, node: &Macro) {
        if self.in_assembly()
            && node.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "assert" | "assert_eq" | "assert_ne" | "panic" | "unreachable"
                )
            })
        {
            self.report("assembly panic shortcut", "panic/assert macro");
        }
        visit::visit_macro(self, node);
    }

    fn visit_local(&mut self, node: &syn::Local) {
        if self.in_assembly()
            && (matches!(node.pat, Pat::Wild(_))
                || pattern_ident(&node.pat).is_some_and(|name| name.starts_with('_')))
            && node
                .init
                .as_ref()
                .is_some_and(|init| self.is_resolved_fallible_call(&init.expr))
        {
            let binding = pattern_ident(&node.pat).unwrap_or_else(|| "_".to_string());
            self.report("discarded assembly Result", &binding);
        }
        visit::visit_local(self, node);
        let callable = node
            .init
            .as_ref()
            .and_then(|init| self.callable_binding_contract(&init.expr));
        if let (Some(name), Some(contract), Some(scope)) = (
            pattern_ident(&node.pat),
            callable,
            self.local_callables.last_mut(),
        ) {
            scope.insert(name, contract);
        }
    }

    fn visit_stmt(&mut self, node: &syn::Stmt) {
        if self.in_assembly()
            && matches!(node, syn::Stmt::Expr(expression, Some(_)) if self.is_resolved_fallible_call(expression))
        {
            self.report("discarded assembly Result", "semicolon expression");
        }
        visit::visit_stmt(self, node);
    }
}

fn attribute_contains_lint(attribute: &syn::Attribute, level: &str, lint: &str) -> bool {
    attribute.path().is_ident(level)
        && attribute.meta.require_list().is_ok_and(|list| {
            list.tokens
                .to_string()
                .split_whitespace()
                .any(|token| token == lint)
        })
}

fn attribute_allows_unused_must_use(attribute: &syn::Attribute) -> bool {
    attribute_contains_lint(attribute, "allow", "unused_must_use")
        || (attribute.path().is_ident("cfg_attr")
            && attribute.meta.require_list().is_ok_and(|list| {
                let tokens = list.tokens.to_string();
                tokens.contains("allow") && tokens.contains("unused_must_use")
            }))
}

#[derive(Default)]
struct UnusedMustUseAllowVisitor {
    found: bool,
}

impl Visit<'_> for UnusedMustUseAllowVisitor {
    fn visit_attribute(&mut self, attribute: &syn::Attribute) {
        self.found |= attribute_allows_unused_must_use(attribute);
    }
}

fn audit_unused_must_use_policy(sources: &[(String, String)]) -> Vec<String> {
    const REQUIRED_DENIES: &[&str] = &[
        "node_system/catalog/mod.rs",
        "node_system/registry/mod.rs",
        "node_system/protocol/mod.rs",
    ];
    let mut offenders = Vec::new();
    for required in REQUIRED_DENIES {
        let Some((_, source)) = sources.iter().find(|(relative, _)| relative == required) else {
            record(
                &mut offenders,
                required,
                1,
                "missing module unused_must_use deny",
                "source missing",
            );
            continue;
        };
        match syn::parse_file(source) {
            Ok(syntax)
                if syntax.attrs.iter().any(|attribute| {
                    attribute_contains_lint(attribute, "deny", "unused_must_use")
                }) => {}
            Ok(_) => record(
                &mut offenders,
                required,
                1,
                "missing module unused_must_use deny",
                "#![deny(unused_must_use)]",
            ),
            Err(error) => record(
                &mut offenders,
                required,
                1,
                "Rust source parse failure",
                &error.to_string(),
            ),
        }
    }
    for (relative, source) in sources {
        if !relative.starts_with("node_system/catalog/")
            && !relative.starts_with("node_system/registry/")
            && !relative.starts_with("node_system/protocol/")
        {
            continue;
        }
        if let Ok(syntax) = syn::parse_file(source) {
            let mut visitor = UnusedMustUseAllowVisitor::default();
            visitor.visit_file(&syntax);
            if visitor.found {
                record(
                    &mut offenders,
                    relative,
                    1,
                    "unused_must_use allow override",
                    "allow(unused_must_use)",
                );
            }
        }
    }
    offenders.sort();
    offenders
}

fn audit_builtin_assembly_sources_from(sources: Vec<(String, String)>) -> Vec<String> {
    let function_index = build_assembly_function_index(&sources);
    let mut offenders = Vec::new();
    for (relative, source) in sources {
        match syn::parse_file(&source) {
            Ok(syntax) => AssemblyEscapeVisitor {
                relative: &relative,
                offenders: &mut offenders,
                assembly_depth: 0,
                audit_all_functions: is_complete_assembly_path(&relative),
                module_path: module_path_for_file(&relative),
                local_callables: Vec::new(),
                function_index: &function_index,
            }
            .visit_file(&syntax),
            Err(error) => record(
                &mut offenders,
                &relative,
                1,
                "Rust source parse failure",
                &error.to_string(),
            ),
        }
    }
    offenders.sort();
    offenders
}

fn audit_builtin_assembly_file(relative: &str, source: &str) -> Vec<String> {
    audit_builtin_assembly_sources_from(vec![(relative.to_owned(), source.to_owned())])
}

fn assembly_sources_under(root: &Path, files: Vec<PathBuf>) -> Vec<(String, String)> {
    files
        .into_iter()
        .map(|file| {
            let relative = file
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let source = std::fs::read_to_string(file).unwrap();
            (relative, source)
        })
        .collect()
}

fn audit_builtin_assembly_tree(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    rust_sources(root, &mut files);
    files.sort();
    audit_builtin_assembly_sources_from(assembly_sources_under(root, files))
}

fn audit_builtin_assembly_sources() -> Vec<String> {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for boundary in [
        "node_system/catalog",
        "node_system/registry",
        "node_system/protocol",
    ] {
        rust_sources(&source_root.join(boundary), &mut files);
    }
    files.retain(|file| {
        let normalized = file.to_string_lossy().replace('\\', "/");
        !normalized.ends_with("tests.rs") && !normalized.contains("/tests/")
    });
    files.extend([
        source_root.join("project/project_store.rs"),
        source_root.join("project/project_state.rs"),
        source_root.join("lib.rs"),
    ]);
    files.sort();
    files.dedup();
    let sources = assembly_sources_under(&source_root, files);
    let mut offenders = audit_unused_must_use_policy(&sources);
    offenders.extend(audit_builtin_assembly_sources_from(sources));
    offenders.sort();
    offenders
}

#[test]
fn builtin_assembly_audit_rejects_panics_fallbacks_and_discarded_results() {
    let offenders = audit_builtin_assembly_file(
        "node_system/catalog/fixture.rs",
        r#"
fn protocol(_: &str, _: Vec<()>) -> Result<(), BuiltinAssemblyError> { Ok(()) }

fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let fallback = ParameterSchema { parameters: Box::new([]) };
    assembled_parameters("yssbi.test", Vec::new()).expect("fallback schema");
    let _ignored = protocol("yssbi.test", Vec::new());
    panic!("assembly failed");
    Ok(ProviderFragment::default())
}

#[cfg(test)]
fn fixture_boundary() {
    build_provider_fragment().expect("test boundary");
}
"#,
    );

    for label in [
        "fallback assembly value",
        "assembly panic shortcut",
        "discarded assembly Result",
    ] {
        assert_offender(&offenders, "fixture.rs", label);
    }
}

#[test]
fn builtin_assembly_audit_covers_real_paths_without_function_name_allowlists() {
    for path in [
        "node_system/registry/mod.rs",
        "node_system/registry/validation.rs",
        "node_system/registry/fingerprint.rs",
        "node_system/catalog/plot/mod.rs",
        "node_system/catalog/localization.rs",
    ] {
        let offenders = audit_builtin_assembly_file(
            path,
            r#"
fn reviewer_named_result() -> Result<(), Error> {
    Ok(())
}

fn reviewer_named_this_something_unexpected() -> Result<(), Error> {
    reviewer_named_result();
    serialize_canonical().expect("canonical registry value");
    Ok(())
}
"#,
        );
        assert_offender(&offenders, path, "assembly panic shortcut");
        assert_offender(&offenders, path, "discarded assembly Result");
    }
}

#[test]
fn builtin_assembly_has_no_escape_hatches() {
    let offenders = audit_builtin_assembly_sources();
    assert!(
        offenders.is_empty(),
        "built-in assembly escape hatches:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn builtin_assembly_audit_resolves_cross_file_calls_without_name_heuristics() {
    let root = audit_fixture("assembly-call-resolution");
    write_fixture(
        &root,
        "node_system/catalog/helpers.rs",
        r#"pub fn imported_result() -> Result<(), Error> { Ok(()) }
pub fn qualified_result() -> Result<(), Error> { Ok(()) }
pub fn wildcard_result() -> Result<(), Error> { Ok(()) }
pub fn ignored_result() -> Result<(), Error> { Ok(()) }
pub fn shadowed() -> Result<(), Error> { Ok(()) }"#,
    );
    write_fixture(
        &root,
        "node_system/catalog/consumer.rs",
        r#"use crate::node_system::catalog::helpers::{
    imported_result as aliased,
    ignored_result,
};

fn consume() -> Result<(), Error> {
    aliased();
    drop(crate::node_system::catalog::helpers::qualified_result());
    let _ = crate::node_system::catalog::helpers::wildcard_result();
    let _ignored = ignored_result();
    Ok(())
}"#,
    );
    write_fixture(
        &root,
        "node_system/catalog/legal.rs",
        r#"use crate::node_system::catalog::helpers::shadowed;

fn consume() -> Result<(), Error> {
    fn shadowed() -> Vec<()> { Vec::new() }
    shadowed();
    Vec::<()>::new();
    drop(Vec::<()>::new());
    let _ = Vec::<()>::new();
    let _ignored = Vec::<()>::new();
    Ok(())
}"#,
    );

    let offenders = audit_builtin_assembly_tree(&root);
    let discarded = offenders
        .iter()
        .filter(|offender| {
            offender.contains("node_system/catalog/consumer.rs")
                && offender.contains("discarded assembly Result")
        })
        .count();
    assert_eq!(
        discarded,
        4,
        "cross-file offenders:\n{}",
        offenders.join("\n")
    );
    assert!(
        offenders
            .iter()
            .all(|offender| !offender.contains("node_system/catalog/legal.rs")),
        "infallible local definitions and Vec::new must be legal:\n{}",
        offenders.join("\n")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn builtin_assembly_audit_requires_module_unused_must_use_denies() {
    let sources = vec![
        (
            "node_system/catalog/mod.rs".to_owned(),
            "mod builtin;".to_owned(),
        ),
        (
            "node_system/registry/mod.rs".to_owned(),
            "#![deny(unused_must_use)]\n#![allow(unused_must_use)]\nmod validation;".to_owned(),
        ),
        (
            "node_system/protocol/mod.rs".to_owned(),
            "#![deny(unused_must_use)]\nmod identity;".to_owned(),
        ),
        (
            "node_system/catalog/builtin.rs".to_owned(),
            "#![allow(unused_must_use)]\nfn assemble() {}".to_owned(),
        ),
    ];

    let offenders = audit_unused_must_use_policy(&sources);
    assert_offender(
        &offenders,
        "node_system/catalog/mod.rs",
        "missing module unused_must_use deny",
    );
    assert_offender(
        &offenders,
        "node_system/registry/mod.rs",
        "unused_must_use allow override",
    );
    assert_offender(
        &offenders,
        "node_system/catalog/builtin.rs",
        "unused_must_use allow override",
    );
    assert!(
        offenders
            .iter()
            .all(|offender| !offender.contains("node_system/protocol/mod.rs")),
        "valid module deny must pass:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn builtin_assembly_audit_resolves_methods_constructors_and_callable_shadows() {
    let root = audit_fixture("assembly-method-constructor-resolution");
    write_fixture(
        &root,
        "node_system/registry/mod.rs",
        r#"pub struct NodeRegistryBuilder;
impl NodeRegistryBuilder {
    pub fn register_provider(&mut self) -> Result<(), Error> { Ok(()) }
    pub fn register_nominal_validator(&mut self) -> Result<(), Error> { Ok(()) }
    pub fn freeze(self) -> Result<(), Error> { Ok(()) }
}"#,
    );
    write_fixture(
        &root,
        "node_system/protocol/identity.rs",
        "semantic_id!(NodeTypeId, \"node type id\", validate);",
    );
    write_fixture(
        &root,
        "node_system/protocol/model.rs",
        r#"pub struct NodeInterfaceProtocol;
impl NodeInterfaceProtocol {
    pub fn new() -> Result<Self, Error> { Ok(Self) }
}"#,
    );
    write_fixture(
        &root,
        "node_system/protocol/parameter.rs",
        r#"pub struct ParameterSchema;
impl ParameterSchema {
    pub fn new() -> Result<Self, Error> { Ok(Self) }
}"#,
    );
    write_fixture(
        &root,
        "node_system/protocol/value.rs",
        r#"pub struct CanonicalDecimal;
impl CanonicalDecimal {
    pub fn new(_: &str) -> Result<Self, Error> { Ok(Self) }
}"#,
    );
    write_fixture(
        &root,
        "node_system/catalog/consumer.rs",
        r#"use crate::node_system::protocol::identity::NodeTypeId;
use crate::node_system::protocol::model::NodeInterfaceProtocol;
use crate::node_system::protocol::parameter::ParameterSchema;
use crate::node_system::protocol::value::CanonicalDecimal;
use crate::node_system::registry::NodeRegistryBuilder;
use crate::node_system::catalog::helpers::imported_result;

fn assemble(builder: &mut NodeRegistryBuilder) -> Result<(), Error> {
    let _ = builder.register_provider();
    let _ignored = builder.register_nominal_validator();
    drop(builder.freeze());
    let _ = NodeTypeId::new("yssbi.test");
    let _ignored = NodeInterfaceProtocol::new();
    drop(ParameterSchema::new());
    let _ = CanonicalDecimal::new("0");

    let imported_result = || Vec::<()>::new();
    let _capacity = imported_result();
    let imported_result = Vec::<()>::new;
    let _capacity = imported_result();
    Ok(())
}"#,
    );
    write_fixture(
        &root,
        "node_system/catalog/helpers.rs",
        "pub fn imported_result() -> Result<(), Error> { Ok(()) }",
    );

    let offenders = audit_builtin_assembly_tree(&root);
    let consumer_offenders = offenders
        .iter()
        .filter(|offender| {
            offender.contains("node_system/catalog/consumer.rs")
                && offender.contains("discarded assembly Result")
        })
        .count();
    assert_eq!(
        consumer_offenders,
        7,
        "reviewer method/constructor offenders:\n{}",
        offenders.join("\n")
    );
    assert!(
        offenders
            .iter()
            .all(|offender| !offender.contains("_capacity")),
        "local callable shadows and Vec::new must remain legal:\n{}",
        offenders.join("\n")
    );
    std::fs::remove_dir_all(root).unwrap();
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
        r#"fn identity(category: &[String], name: &str) -> NodeTypeId {
    NodeTypeId::new(format!("{}:{}", category.join(":"), name))
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
    let node_type_id = NodeTypeId::new(format!("{category:?}:{name}"));
    registry.insert(format!("{}:{}", categories.join(":"), title), node_type_id);
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
fn audit_rejects_union_inline_module_and_visibility_variants() {
    let root = audit_fixture("structural-variants");
    write_fixture(&root, "union.rs", "pub union NodeRegistry { value: usize }");
    write_fixture(
        &root,
        "inline.rs",
        r#"mod graph {
    pub(crate) mod register { pub struct Hidden; }
    pub(super) mod core { pub struct Runtime; }
    mod infer { pub struct Inference; }
}
mod execution { pub(in crate) mod engine { pub struct Hidden; } }"#,
    );
    write_fixture(
        &root,
        "visibility.rs",
        r#"use crate::graph::register::Private;
pub(crate) use crate::execution::engine::CrateVisible;
pub(super) use crate::graph::core::ParentVisible;
pub(in crate) use crate::graph::infer::Scoped;"#,
    );

    let offenders = audit_source_tree(&root, None);
    assert_offender(&offenders, "union.rs", "second NodeRegistry definition");
    for label in [
        "graph Registry path",
        "old graph core path",
        "old graph inference path",
        "old execution engine path",
    ] {
        assert_offender(&offenders, "inline.rs", label);
    }
    for label in [
        "graph Registry path",
        "old execution engine path",
        "old graph core path",
        "old graph inference path",
    ] {
        assert_offender(&offenders, "visibility.rs", label);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_rejects_macro_generated_architecture_and_legacy_include_expressions() {
    let root = audit_fixture("macro-generated");
    write_fixture(
        &root,
        "generated.rs",
        r#"macro_rules! legacy_registry {
    () => {
        pub struct NodeRegistry;
        fn legacy(graph: GraphInstance, definition: NodeDefinition) {
            crate::graph::register::register(graph, definition);
            crate::execution::engine::Executor::new();
            resolve_dynamic_pins();
        }
    };
}
legacy_registry!();"#,
    );
    write_fixture(
        &root,
        "static_include.rs",
        r#"include!(concat!("graph/", "register/", "mod.rs"));"#,
    );
    write_fixture(
        &root,
        "runtime_include.rs",
        r#"include!(runtime_path!("execution", "engine", "mod.rs"));"#,
    );

    let offenders = audit_source_tree(&root, None);
    for label in [
        "macro NodeRegistry definition",
        "legacy GraphInstance",
        "legacy node definition",
        "graph Registry path",
        "old execution engine path",
        "dynamic pin resolution",
    ] {
        assert_offender(&offenders, "generated.rs", label);
    }
    assert_offender(&offenders, "static_include.rs", "legacy module include");
    assert_offender(&offenders, "runtime_include.rs", "legacy module include");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_allows_breadcrumbs_logs_and_nonlegacy_include_expressions() {
    let root = audit_fixture("legal-identity-and-includes");
    write_fixture(
        &root,
        "legal.rs",
        r#"fn breadcrumb(category: &[String], name: &str) -> String {
    let path = category.join(":");
    tracing::debug!("category path {} for {}", path, name);
    format!("{}:{}", category.join(":"), name)
}
include!("current/generated_values.rs");
include!(concat!("current/", "generated_values.rs"));"#,
    );

    let offenders = audit_source_tree(&root, None);
    assert!(
        offenders.is_empty(),
        "legal breadcrumbs, logs, and includes must not be rejected:\n{}",
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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, Item, Lit, Macro, Member, Meta, Pat, ReturnType, Token, Type, UseTree};

const REGISTRY_AUTHORITY: &str = "node_system/registry/model.rs";

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
        UseTree::Glob(_) => {
            let mut path = prefix.clone();
            path.push("*".to_owned());
            paths.push(path);
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum MacroToken {
    Ident(String),
}

fn flatten_macro_tokens(tokens: TokenStream, flattened: &mut Vec<MacroToken>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => flatten_macro_tokens(group.stream(), flattened),
            TokenTree::Ident(ident) => flattened.push(MacroToken::Ident(ident.to_string())),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
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

fn is_pin_definition_name_base(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "pin_definition"),
        Expr::Field(field) => {
            matches!(&field.member, Member::Named(name) if name == "definition")
                && expr_mentions(&field.base, &["pin"])
        }
        Expr::Paren(expression) => is_pin_definition_name_base(&expression.expr),
        Expr::Group(expression) => is_pin_definition_name_base(&expression.expr),
        Expr::Reference(expression) => is_pin_definition_name_base(&expression.expr),
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
        let mut flattened = Vec::new();
        flatten_macro_tokens(mac.tokens.clone(), &mut flattened);
        if macro_defines_node_registry(&flattened) {
            self.report("macro NodeRegistry definition", "NodeRegistry");
        }
    }
}

impl<'ast> Visit<'ast> for SourceVisitor<'_> {
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
            _ => {}
        }
        visit::visit_item(self, node);
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

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
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
            && is_pin_definition_name_base(&node.base)
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

    fn visit_macro(&mut self, node: &'ast Macro) {
        self.inspect_macro(node);
        visit::visit_macro(self, node);
    }
}

fn inspect_source(relative: &str, source: &str, offenders: &mut Vec<String>) {
    match syn::parse_file(source) {
        Ok(file) => SourceVisitor {
            relative,
            source,
            offenders,
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

fn cfg_predicate_is_exclusively_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("all") || list.path.is_ident("any") => {
            let Ok(predicates) =
                Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
            else {
                return false;
            };
            if list.path.is_ident("all") {
                predicates.iter().any(cfg_predicate_is_exclusively_test)
            } else {
                !predicates.is_empty() && predicates.iter().all(cfg_predicate_is_exclusively_test)
            }
        }
        Meta::List(_) | Meta::NameValue(_) => false,
    }
}

fn is_test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if attribute.path().is_ident("test") {
            return true;
        }
        let Ok(cfg) = attribute.meta.require_list() else {
            return false;
        };
        if !attribute.path().is_ident("cfg") {
            return false;
        }
        let Ok(predicates) =
            Punctuated::<Meta, Token![,]>::parse_terminated.parse2(cfg.tokens.clone())
        else {
            return false;
        };
        predicates.len() == 1
            && predicates
                .first()
                .is_some_and(cfg_predicate_is_exclusively_test)
    })
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

#[derive(Default)]
struct CompilerDiagnosticAudit {
    violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CallableContext {
    Module(String),
    Impl(String),
    Trait(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CallableKey {
    file: String,
    module: String,
    context: CallableContext,
    name: String,
}

#[derive(Clone)]
struct DiagnosticConstructorCandidate {
    key: CallableKey,
    static_parameters: HashMap<String, usize>,
    output: ReturnType,
    body: Option<syn::Block>,
}

#[derive(Default)]
struct CompilerDiagnosticSyntaxIndex {
    diagnostic_types: HashSet<String>,

    constructors: HashMap<CallableKey, usize>,
}

struct DiagnosticDefinitionCollector {
    file: String,
    modules: Vec<String>,
    owner: Option<CallableContext>,
    diagnostic_types: HashSet<String>,
    aliases: Vec<(String, Type)>,
    import_aliases: Vec<(String, String)>,
    constructors: Vec<DiagnosticConstructorCandidate>,
}

fn static_string_parameters(signature: &syn::Signature) -> HashMap<String, usize> {
    signature
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(argument) => Some(argument),
        })
        .enumerate()
        .filter_map(|(position, argument)| {
            type_is_static_str_reference(&argument.ty)
                .then(|| pattern_ident(&argument.pat).map(|name| (name, position)))
                .flatten()
        })
        .collect()
}

fn module_context(modules: &[String]) -> CallableContext {
    CallableContext::Module(modules.join("::"))
}

fn callable_key(
    file: &str,
    modules: &[String],
    owner: Option<&CallableContext>,
    name: &syn::Ident,
) -> CallableKey {
    CallableKey {
        file: file.to_owned(),
        module: modules.join("::"),
        context: owner.cloned().unwrap_or_else(|| module_context(modules)),
        name: name.to_string(),
    }
}

fn impl_context(self_type: &Type) -> Option<CallableContext> {
    named_type(self_type).map(CallableContext::Impl)
}

fn named_type(value_type: &Type) -> Option<String> {
    match value_type {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => named_type(&reference.elem),
        Type::Paren(parenthesized) => named_type(&parenthesized.elem),
        Type::Group(group) => named_type(&group.elem),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for DiagnosticDefinitionCollector {
    fn visit_item(&mut self, node: &'ast Item) {
        if is_test_only(item_attributes(node)) {
            return;
        }
        visit::visit_item(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let code = node.fields.iter().any(|field| {
            field.ident.as_ref().is_some_and(|name| name == "code")
                && type_is_static_str_reference(&field.ty)
        });
        let detail = node
            .fields
            .iter()
            .any(|field| field.ident.as_ref().is_some_and(|name| name == "detail"));
        if code && detail {
            self.diagnostic_types.insert(node.ident.to_string());
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.aliases
            .push((node.ident.to_string(), (*node.ty).clone()));
        visit::visit_item_type(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let mut bindings = Vec::new();
        collect_use_bindings(&node.tree, &mut Vec::new(), &mut bindings);
        self.import_aliases.extend(
            bindings
                .into_iter()
                .filter_map(|(alias, target)| target.last().cloned().map(|target| (alias, target))),
        );
        visit::visit_item_use(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.modules.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.modules.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.owner.clone();
        self.owner = impl_context(&node.self_ty);
        visit::visit_item_impl(self, node);
        self.owner = previous;
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let previous = self.owner.clone();
        self.owner = Some(CallableContext::Trait(node.ident.to_string()));
        visit::visit_item_trait(self, node);
        self.owner = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: Some((*node.block).clone()),
        });
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: Some(node.block.clone()),
        });
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: node.default.clone(),
        });
        visit::visit_trait_item_fn(self, node);
    }
}

fn resolve_path_callable(
    current: &CallableKey,
    path: &syn::Path,
    definitions: &HashSet<CallableKey>,
) -> Option<CallableKey> {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let name = segments.last()?.clone();
    let mut candidates = Vec::new();
    if segments.len() == 1 {
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Module(current.module.clone()),
            name,
        });
    } else {
        let qualifier = &segments[segments.len() - 2];
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Impl(qualifier.clone()),
            name: name.clone(),
        });
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Trait(qualifier.clone()),
            name: name.clone(),
        });
        let mut modules = current
            .module
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        modules.extend(segments[..segments.len() - 1].iter().cloned());
        let module = modules.join("::");
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: module.clone(),
            context: CallableContext::Module(module),
            name,
        });
    }
    let mut matches = candidates
        .into_iter()
        .filter(|candidate| definitions.contains(candidate));
    let resolved = matches.next()?;
    matches.next().is_none().then_some(resolved)
}

fn resolve_method_callable(
    current: &CallableKey,
    receiver: &Expr,
    receiver_type: Option<&str>,
    method: &syn::Ident,
    definitions: &HashSet<CallableKey>,
) -> Option<CallableKey> {
    if matches!(receiver, Expr::Path(path) if path.path.is_ident("self")) {
        let candidate = CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: current.context.clone(),
            name: method.to_string(),
        };
        return definitions.contains(&candidate).then_some(candidate);
    }
    let receiver_type = receiver_type?;
    let candidate = CallableKey {
        file: current.file.clone(),
        module: current.module.clone(),
        context: CallableContext::Impl(receiver_type.to_owned()),
        name: method.to_string(),
    };
    definitions.contains(&candidate).then_some(candidate)
}

fn declared_return_type(candidate: &DiagnosticConstructorCandidate) -> Option<String> {
    let ReturnType::Type(_, value_type) = &candidate.output else {
        return None;
    };
    let name = named_type(value_type)?;
    if name != "Self" {
        return Some(name);
    }
    match &candidate.key.context {
        CallableContext::Impl(owner) => Some(owner.clone()),
        CallableContext::Module(_) | CallableContext::Trait(_) => None,
    }
}

fn inferred_expression_type(
    expression: &Expr,
    current: Option<&CallableKey>,
    definitions: &HashSet<CallableKey>,
    return_types: &HashMap<CallableKey, String>,
) -> Option<String> {
    match expression {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .filter(|name| name.chars().next().is_some_and(char::is_uppercase)),
        Expr::Call(call) => {
            let current = current?;
            let Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let callee = resolve_path_callable(current, &path.path, definitions)?;
            return_types.get(&callee).cloned()
        }
        Expr::Struct(value) => value
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Paren(value) => {
            inferred_expression_type(&value.expr, current, definitions, return_types)
        }
        Expr::Group(value) => {
            inferred_expression_type(&value.expr, current, definitions, return_types)
        }
        _ => None,
    }
}

fn local_receiver_type(
    pattern: &Pat,
    expression: &Expr,
    current: Option<&CallableKey>,
    definitions: &HashSet<CallableKey>,
    return_types: &HashMap<CallableKey, String>,
) -> Option<String> {
    match pattern {
        Pat::Type(typed) => named_type(&typed.ty),
        _ => inferred_expression_type(expression, current, definitions, return_types),
    }
}

fn unwrap_diagnostic_code_expression(expression: &Expr) -> &Expr {
    match expression {
        Expr::Call(call)
            if matches!(
                call.func.as_ref(),
                Expr::Path(path)
                    if path.path.segments.last().is_some_and(|segment| segment.ident == "new")
                        && path.path.segments.iter().any(|segment| segment.ident == "DiagnosticCode")
            ) =>
        {
            call.args
                .first()
                .map_or(expression, unwrap_diagnostic_code_expression)
        }
        Expr::Paren(parenthesized) => unwrap_diagnostic_code_expression(&parenthesized.expr),
        Expr::Group(group) => unwrap_diagnostic_code_expression(&group.expr),
        _ => expression,
    }
}

fn parameter_position_in_expression(
    expression: &Expr,
    parameters: &HashMap<String, usize>,
    bindings: &HashMap<String, Expr>,
    visiting: &mut HashSet<String>,
) -> Option<usize> {
    let expression = unwrap_diagnostic_code_expression(expression);
    match expression {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            let name = path.path.segments[0].ident.to_string();
            if let Some(position) = parameters.get(&name) {
                return Some(*position);
            }
            if !visiting.insert(name.clone()) {
                return None;
            }
            let resolved = bindings.get(&name).and_then(|bound| {
                parameter_position_in_expression(bound, parameters, bindings, visiting)
            });
            visiting.remove(&name);
            resolved
        }
        Expr::Reference(reference) => {
            parameter_position_in_expression(&reference.expr, parameters, bindings, visiting)
        }
        Expr::Paren(parenthesized) => {
            parameter_position_in_expression(&parenthesized.expr, parameters, bindings, visiting)
        }
        Expr::Group(group) => {
            parameter_position_in_expression(&group.expr, parameters, bindings, visiting)
        }
        _ => None,
    }
}

struct ConstructorFlowAnalyzer<'a> {
    current: &'a CallableKey,
    parameters: &'a HashMap<String, usize>,
    diagnostic_types: &'a HashSet<String>,
    definitions: &'a HashSet<CallableKey>,
    constructors: &'a HashMap<CallableKey, usize>,
    return_types: &'a HashMap<CallableKey, String>,
    bindings: HashMap<String, Expr>,
    receiver_types: HashMap<String, String>,
    positions: HashSet<usize>,
}

impl ConstructorFlowAnalyzer<'_> {
    fn record_source(&mut self, expression: &Expr) {
        if let Some(position) = parameter_position_in_expression(
            expression,
            self.parameters,
            &self.bindings,
            &mut HashSet::new(),
        ) {
            self.positions.insert(position);
        }
    }
}

impl<'ast> Visit<'ast> for ConstructorFlowAnalyzer<'_> {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        if let (Some(name), Some(init)) = (pattern_ident(&node.pat), node.init.as_ref()) {
            if let Some(receiver_type) = local_receiver_type(
                &node.pat,
                &init.expr,
                Some(self.current),
                self.definitions,
                self.return_types,
            ) {
                self.receiver_types.insert(name.clone(), receiver_type);
            }
            self.bindings.insert(name, (*init.expr).clone());
        }
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let diagnostic = node
            .path
            .segments
            .last()
            .is_some_and(|segment| self.diagnostic_types.contains(&segment.ident.to_string()));
        if diagnostic {
            for field in &node.fields {
                if matches!(&field.member, Member::Named(name) if name == "code") {
                    self.record_source(&field.expr);
                }
            }
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if let Some(callee) = resolve_path_callable(self.current, &path.path, self.definitions)
            {
                if let Some(position) = self.constructors.get(&callee) {
                    if let Some(argument) = node.args.iter().nth(*position) {
                        self.record_source(argument);
                    }
                }
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let receiver_type = match node.receiver.as_ref() {
            Expr::Path(path) if path.path.segments.len() == 1 => self
                .receiver_types
                .get(&path.path.segments[0].ident.to_string())
                .map(String::as_str),
            _ => None,
        };
        if let Some(callee) = resolve_method_callable(
            self.current,
            &node.receiver,
            receiver_type,
            &node.method,
            self.definitions,
        ) {
            if let Some(position) = self.constructors.get(&callee) {
                if let Some(argument) = node.args.iter().nth(*position) {
                    self.record_source(argument);
                }
            }
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn build_compiler_diagnostic_index(files: &[(String, syn::File)]) -> CompilerDiagnosticSyntaxIndex {
    let mut collector = DiagnosticDefinitionCollector {
        file: String::new(),
        modules: Vec::new(),
        owner: None,
        diagnostic_types: HashSet::from(["NodeDiagnostic".to_owned()]),
        aliases: Vec::new(),
        import_aliases: Vec::new(),
        constructors: Vec::new(),
    };
    for (relative, file) in files {
        collector.file.clone_from(relative);
        collector.modules.clear();
        collector.owner = None;
        collector.visit_file(file);
    }

    loop {
        let mut changed = false;
        for (alias, target) in &collector.aliases {
            if named_type(target).is_some_and(|name| collector.diagnostic_types.contains(&name)) {
                changed |= collector.diagnostic_types.insert(alias.clone());
            }
        }
        for (alias, target) in &collector.import_aliases {
            if collector.diagnostic_types.contains(target) {
                changed |= collector.diagnostic_types.insert(alias.clone());
            }
        }
        if !changed {
            break;
        }
    }

    let definitions = collector
        .constructors
        .iter()
        .map(|candidate| candidate.key.clone())
        .collect::<HashSet<_>>();
    let return_types = collector
        .constructors
        .iter()
        .filter_map(|candidate| {
            declared_return_type(candidate).map(|output| (candidate.key.clone(), output))
        })
        .collect::<HashMap<_, _>>();
    let mut constructors = HashMap::<CallableKey, usize>::new();
    loop {
        let mut changed = false;
        for candidate in &collector.constructors {
            let Some(body) = &candidate.body else {
                continue;
            };
            if candidate.static_parameters.is_empty() {
                continue;
            }
            let mut analyzer = ConstructorFlowAnalyzer {
                current: &candidate.key,
                parameters: &candidate.static_parameters,
                diagnostic_types: &collector.diagnostic_types,
                definitions: &definitions,
                constructors: &constructors,
                return_types: &return_types,
                bindings: HashMap::new(),
                receiver_types: HashMap::new(),
                positions: HashSet::new(),
            };
            analyzer.visit_block(body);
            if analyzer.positions.len() == 1 {
                let position = *analyzer.positions.iter().next().unwrap();
                changed |= constructors
                    .insert(candidate.key.clone(), position)
                    .is_none();
            }
        }
        if !changed {
            break;
        }
    }

    CompilerDiagnosticSyntaxIndex {
        diagnostic_types: collector.diagnostic_types,
        constructors,
    }
}

struct CompilerDiagnosticVisitor<'a> {
    relative: &'a str,
    source: &'a str,
    audit: &'a mut CompilerDiagnosticAudit,
    index: &'a CompilerDiagnosticSyntaxIndex,
    argument_maps: Vec<HashSet<String>>,
    modules: Vec<String>,
    owner: Option<CallableContext>,
}

impl CompilerDiagnosticVisitor<'_> {
    fn report(&mut self, label: &str, token: &str) {
        record(
            &mut self.audit.violations,
            self.relative,
            line_for(self.source, token),
            label,
            token,
        );
    }

    fn inspect_string(&mut self, value: &str) {
        if value.starts_with("compiler.") {
            self.report("untyped compiler diagnostic code", value);
        }
    }

    fn receiver_is_argument_map(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                self.argument_maps
                    .iter()
                    .rev()
                    .any(|scope| scope.contains(&name))
            }
            Expr::Field(field) => matches!(
                &field.member,
                Member::Named(name)
                    if matches!(name.to_string().as_str(), "arguments" | "diagnostic_arguments")
            ),
            Expr::Paren(parenthesized) => self.receiver_is_argument_map(&parenthesized.expr),
            Expr::Reference(reference) => self.receiver_is_argument_map(&reference.expr),
            _ => false,
        }
    }

    fn inspect_macro_tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => self.inspect_macro_tokens(group.stream()),
                TokenTree::Literal(literal) => {
                    if let Ok(value) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        self.inspect_string(&value.value());
                    }
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
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

fn type_is_static_str_reference(value_type: &Type) -> bool {
    let Type::Reference(reference) = value_type else {
        return false;
    };
    reference
        .lifetime
        .as_ref()
        .is_some_and(|lifetime| lifetime.ident == "static")
        && matches!(
            reference.elem.as_ref(),
            Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "str")
        )
}

fn path_is_argument_map_constructor(path: &syn::Path) -> bool {
    path.segments.iter().any(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "DiagnosticArguments" | "BTreeMap"
        )
    })
}

fn type_is_argument_map(value_type: &Type) -> bool {
    matches!(
        value_type,
        Type::Path(path) if path_is_argument_map_constructor(&path.path)
    )
}

fn expression_constructs_argument_map(expression: &Expr) -> bool {
    match expression {
        Expr::Call(call) => matches!(
            call.func.as_ref(),
            Expr::Path(path)
                if path_is_argument_map_constructor(&path.path)
                    && path.path.segments.last().is_some_and(|segment| {
                        matches!(segment.ident.to_string().as_str(), "new" | "default" | "from")
                    })
        ),
        Expr::Macro(expression) => expression.mac.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "btreemap" | "diagnostic_arguments"
            )
        }),
        Expr::Paren(parenthesized) => expression_constructs_argument_map(&parenthesized.expr),
        Expr::Group(group) => expression_constructs_argument_map(&group.expr),
        _ => false,
    }
}

fn expression_is_detail_literal(expression: &Expr) -> bool {
    if static_string_expression(expression).as_deref() == Some("detail") {
        return true;
    }
    match expression {
        Expr::Call(call)
            if matches!(
                call.func.as_ref(),
                Expr::Path(path)
                    if path.path.segments.last().is_some_and(|segment| {
                        matches!(segment.ident.to_string().as_str(), "from" | "new")
                    })
            ) =>
        {
            call.args.first().is_some_and(expression_is_detail_literal)
        }
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "into" | "to_owned" | "to_string"
            ) =>
        {
            expression_is_detail_literal(&call.receiver)
        }
        Expr::Reference(reference) => expression_is_detail_literal(&reference.expr),
        Expr::Paren(parenthesized) => expression_is_detail_literal(&parenthesized.expr),
        Expr::Group(group) => expression_is_detail_literal(&group.expr),
        _ => false,
    }
}

fn expression_contains_detail_entry(expression: &Expr) -> bool {
    match expression {
        Expr::Tuple(tuple) => tuple
            .elems
            .first()
            .is_some_and(expression_is_detail_literal),
        Expr::Array(array) => array.elems.iter().any(expression_contains_detail_entry),
        Expr::Reference(reference) => expression_contains_detail_entry(&reference.expr),
        Expr::Paren(parenthesized) => expression_contains_detail_entry(&parenthesized.expr),
        Expr::Group(group) => expression_contains_detail_entry(&group.expr),
        _ => false,
    }
}

fn pattern_argument_map_name(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Type(typed) if type_is_argument_map(&typed.ty) => pattern_ident(&typed.pat),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for CompilerDiagnosticVisitor<'_> {
    fn visit_item(&mut self, node: &'ast Item) {
        if is_test_only(item_attributes(node)) {
            return;
        }
        visit::visit_item(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.modules.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.modules.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.owner.clone();
        self.owner = impl_context(&node.self_ty);
        visit::visit_item_impl(self, node);
        self.owner = previous;
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let previous = self.owner.clone();
        self.owner = Some(CallableContext::Trait(node.ident.to_string()));
        visit::visit_item_trait(self, node);
        self.owner = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if self
            .index
            .diagnostic_types
            .contains(&node.ident.to_string())
        {
            for field in &node.fields {
                let Some(name) = &field.ident else {
                    continue;
                };
                if matches!(name.to_string().as_str(), "code" | "detail") {
                    self.report("untyped compiler issue field", &name.to_string());
                }
            }
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let type_name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if type_name
            .as_ref()
            .is_some_and(|name| self.index.diagnostic_types.contains(name))
        {
            let name = type_name.as_deref().unwrap_or("NodeDiagnostic");
            self.report(
                "direct compiler NodeDiagnostic construction",
                &format!("{name} {{"),
            );
        }

        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if path_is_argument_map_constructor(&path.path)
                && path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "from")
                && node.args.iter().any(expression_contains_detail_entry)
            {
                self.report("generic compiler diagnostic argument", "\"detail\"");
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "insert"
            && self.receiver_is_argument_map(&node.receiver)
            && node.args.first().is_some_and(expression_is_detail_literal)
        {
            self.report("generic compiler diagnostic argument", "\"detail\"");
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        let name = pattern_argument_map_name(&node.pat).or_else(|| {
            let init = node.init.as_ref()?;
            expression_constructs_argument_map(&init.expr)
                .then(|| pattern_ident(&node.pat))
                .flatten()
        });
        if let (Some(name), Some(scope)) = (name, self.argument_maps.last_mut()) {
            scope.insert(name);
        }
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.argument_maps.push(HashSet::new());
        visit::visit_block(self, node);
        self.argument_maps.pop();
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.inspect_string(&node.value());
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if matches!(
            name.as_deref(),
            Some(
                "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "debug_assert"
                    | "debug_assert_eq"
                    | "debug_assert_ne"
            )
        ) {
            return;
        }
        self.inspect_macro_tokens(node.tokens.clone());
    }
}

fn is_compiler_test_source(relative: &str) -> bool {
    let file_name = relative.rsplit('/').next().unwrap_or(relative);
    file_name == "tests.rs" || file_name.starts_with("tests_") || file_name.ends_with("_tests.rs")
}

fn inspect_compiler_diagnostic_source(
    relative: &str,
    source: &str,
    audit: &mut CompilerDiagnosticAudit,
) {
    match syn::parse_file(source) {
        Ok(module) => {
            let index = build_compiler_diagnostic_index(&[(relative.to_owned(), module.clone())]);
            CompilerDiagnosticVisitor {
                relative,
                source,
                audit,
                index: &index,
                argument_maps: Vec::new(),
                modules: Vec::new(),
                owner: None,
            }
            .visit_file(&module);
        }
        Err(error) => record(
            &mut audit.violations,
            relative,
            1,
            "Rust source parse failure",
            &error.to_string(),
        ),
    }
}

fn audit_compiler_diagnostic_tree(
    compiler_root: &Path,
    exclude_definition_authority: bool,
) -> CompilerDiagnosticAudit {
    let mut paths = Vec::new();
    rust_sources(compiler_root, &mut paths);
    paths.sort();

    let mut audit = CompilerDiagnosticAudit::default();
    let mut sources = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(compiler_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_compiler_test_source(&relative)
            || exclude_definition_authority && relative == "diagnostics.rs"
        {
            continue;
        }
        let source = std::fs::read_to_string(path).unwrap();
        match syn::parse_file(&source) {
            Ok(module) => sources.push((relative, source, module)),
            Err(error) => record(
                &mut audit.violations,
                &relative,
                1,
                "Rust source parse failure",
                &error.to_string(),
            ),
        }
    }

    let indexed_sources = sources
        .iter()
        .map(|(relative, _, module)| (relative.clone(), module.clone()))
        .collect::<Vec<_>>();
    let index = build_compiler_diagnostic_index(&indexed_sources);
    for (relative, source, module) in &sources {
        CompilerDiagnosticVisitor {
            relative,
            source,
            audit: &mut audit,
            index: &index,
            argument_maps: Vec::new(),
            modules: Vec::new(),
            owner: None,
        }
        .visit_file(module);
    }
    audit.violations.sort();
    audit.violations.dedup();
    audit
}

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

fn audit_raw_graph_document_mutations(transaction_source: &str) -> Vec<String> {
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

fn audit_production_graph_write_surface(
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
    let violations = audit_production_graph_write_surface(
        &std::fs::read_to_string(source_root.join("node_system/document/mod.rs")).unwrap(),
        &std::fs::read_to_string(source_root.join("node_system/document/mutation.rs")).unwrap(),
        &std::fs::read_to_string(source_root.join("project/project_state.rs")).unwrap(),
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
        "fn legacy(pin: Pin) { let _ = pin.definition.name; }",
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

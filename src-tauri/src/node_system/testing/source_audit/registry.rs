use std::collections::{HashMap, HashSet};
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, Item, Lit, Macro, Member, Pat, ReturnType, Type};

use super::shared::{line_for, macro_arguments, pattern_ident, record, rust_sources};

const REGISTRY_AUTHORITY: &str = "node_system/registry/model.rs";

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

pub(super) fn audit_source_tree(root: &Path, excluded_relative: Option<&str>) -> Vec<String> {
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

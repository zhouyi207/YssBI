use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, ImplItem, Item, TraitItem};

use super::DependencyViolation;
use crate::test_support::source_audit::{expand_use_tree, is_test_only, normalized_ident};

pub(super) struct ForbiddenDependencyVisitor<'a> {
    pub(super) forbidden_module: &'a str,
    pub(super) file: &'a str,
    pub(super) module: &'a str,
    pub(super) violations: &'a mut BTreeSet<DependencyViolation>,
}

impl ForbiddenDependencyVisitor<'_> {
    fn record(&mut self, reference: String) {
        self.violations.insert(DependencyViolation {
            file: self.file.to_owned(),
            module: self.module.to_owned(),
            reference,
        });
    }

    fn inspect_segments(&mut self, segments: impl IntoIterator<Item = String>) {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments
            .iter()
            .any(|segment| segment == self.forbidden_module)
        {
            self.record(segments.join("::"));
        }
    }
}

impl<'ast> Visit<'ast> for ForbiddenDependencyVisitor<'_> {
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

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if is_test_only(expr_attributes(expr)) {
            return;
        }
        visit::visit_expr(self, expr);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if is_test_only(&local.attrs) {
            return;
        }
        visit::visit_local(self, local);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if is_test_only(&field.attrs) {
            return;
        }
        visit::visit_field(self, field);
    }

    fn visit_variant(&mut self, variant: &'ast syn::Variant) {
        if is_test_only(&variant.attrs) {
            return;
        }
        visit::visit_variant(self, variant);
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if is_test_only(&arm.attrs) {
            return;
        }
        visit::visit_arm(self, arm);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if is_test_only(&item.attrs) {
            return;
        }
        let mut paths = Vec::new();
        expand_use_tree(&item.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.inspect_segments(path);
        }
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.inspect_segments(
            path.segments
                .iter()
                .map(|segment| normalized_ident(&segment.ident)),
        );
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|segment| normalized_ident(&segment.ident) == "include")
        {
            self.record("macro-include!::<unexpanded>".to_owned());
        }
        if token_stream_contains_path_ident(&mac.tokens, self.forbidden_module) {
            self.record(format!("macro-token::{}", self.forbidden_module));
        }
        visit::visit_macro(self, mac);
    }
}

fn token_stream_contains_path_ident(tokens: &TokenStream, expected: &str) -> bool {
    let tokens = tokens.clone().into_iter().collect::<Vec<_>>();
    tokens.iter().enumerate().any(|(index, token)| match token {
        TokenTree::Group(group) => token_stream_contains_path_ident(&group.stream(), expected),
        TokenTree::Ident(ident) if normalized_ident(ident) == expected => {
            has_path_separator_before(&tokens, index) || has_path_separator_after(&tokens, index)
        }
        TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Punct(_) => false,
    })
}

fn has_path_separator_before(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2 && is_path_separator(&tokens[index - 2..index])
}

fn has_path_separator_after(tokens: &[TokenTree], index: usize) -> bool {
    tokens
        .get(index + 1..index + 3)
        .is_some_and(is_path_separator)
}

fn is_path_separator(tokens: &[TokenTree]) -> bool {
    matches!(tokens, [TokenTree::Punct(first), TokenTree::Punct(second)] if first.as_char() == ':' && second.as_char() == ':')
}

pub(super) fn item_attributes(item: &Item) -> &[syn::Attribute] {
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

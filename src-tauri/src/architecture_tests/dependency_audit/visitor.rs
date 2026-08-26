use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, ImplItem, Item, LitStr, TraitItem};

use super::DependencyViolation;
use crate::architecture_tests::model::{RawDependency, RustDependencyKind, RustDependencyMode};
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

pub(super) struct RawDependencyVisitor<'a> {
    package: &'a str,
    file: &'a str,
    owner: &'a str,
    mode: RustDependencyMode,
    source: &'a str,
    cursor: usize,
    unresolved_include: Option<String>,
    code_includes: Vec<String>,
    dependencies: &'a mut Vec<RawDependency>,
}

impl<'a> RawDependencyVisitor<'a> {
    pub(super) fn new(
        package: &'a str,
        file: &'a str,
        owner: &'a str,
        mode: RustDependencyMode,
        source: &'a str,
        dependencies: &'a mut Vec<RawDependency>,
    ) -> Self {
        Self {
            package,
            file,
            owner,
            mode,
            source,
            cursor: 0,
            unresolved_include: None,
            code_includes: Vec::new(),
            dependencies,
        }
    }

    pub(super) fn unresolved_include(&self) -> Option<String> {
        self.unresolved_include.clone()
    }

    pub(super) fn code_includes(&self) -> Vec<String> {
        self.code_includes.clone()
    }

    fn record(&mut self, kind: RustDependencyKind, written_target: impl Into<String>) {
        let written_target = written_target.into();
        let (line, column) = self.location_for(&written_target);
        self.dependencies.push(RawDependency {
            owning_package: self.package.to_owned(),
            repository_relative_source_file: self.file.to_owned(),
            fully_qualified_owner: self.owner.to_owned(),
            kind,
            mode: self.mode,
            written_target,
            line,
            column,
        });
    }

    fn location_for(&mut self, target: &str) -> (usize, usize) {
        let (next_cursor, line, column) = source_location(self.source, target, self.cursor);
        self.cursor = next_cursor;
        (line, column)
    }
}

pub(super) fn source_location(source: &str, target: &str, cursor: usize) -> (usize, usize, usize) {
    let search_start = cursor.min(source.len());
    let target_start = target
        .find("::")
        .map(|index| &target[..index])
        .unwrap_or(target);
    let (index, matched_length) = source[search_start..]
        .find(target)
        .map(|index| (search_start + index, target.len()))
        .or_else(|| {
            source[search_start..]
                .find(target_start)
                .map(|index| (search_start + index, target_start.len()))
        })
        .unwrap_or((search_start, 0));
    let prefix = &source[..index];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit('\n')
        .next()
        .map_or(1, |line| line.chars().count() + 1);
    (index.saturating_add(matched_length), line, column)
}

impl<'ast> Visit<'ast> for RawDependencyVisitor<'_> {
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

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if is_test_only(std::slice::from_ref(attribute)) {
            return;
        }
        let path = attribute.path();
        if path.segments.len() > 1
            && path
                .segments
                .first()
                .is_some_and(|segment| is_dependency_root(&normalized_ident(&segment.ident)))
        {
            self.record(RustDependencyKind::Path, path_to_string(path));
        }
        match &attribute.meta {
            syn::Meta::Path(_) => {}
            syn::Meta::List(list) if !is_lint_level_attribute(path) => {
                for path in token_stream_paths(&list.tokens) {
                    self.record(RustDependencyKind::Path, path);
                }
            }
            syn::Meta::List(_) => {}
            syn::Meta::NameValue(value) => self.visit_expr(&value.value),
        }
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if is_test_only(&item.attrs) {
            return;
        }
        let kind = if matches!(item.vis, syn::Visibility::Public(_)) {
            RustDependencyKind::ReExport
        } else {
            RustDependencyKind::Use
        };
        let mut paths = Vec::new();
        expand_use_tree(&item.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.record(kind, path.join("::"));
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if is_test_only(&item.attrs) {
            return;
        }
        self.record(RustDependencyKind::Use, normalized_ident(&item.ident));
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path.segments.len() > 1
            && path
                .segments
                .first()
                .is_some_and(|segment| is_dependency_root(&normalized_ident(&segment.ident)))
        {
            self.record(RustDependencyKind::Path, path_to_string(path));
        }
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let target = path_to_string(&mac.path);
        let macro_name = mac
            .path
            .segments
            .last()
            .map(|segment| normalized_ident(&segment.ident));
        if macro_name
            .as_deref()
            .is_some_and(|name| matches!(name, "include" | "include_str" | "include_bytes"))
        {
            let include_target = syn::parse2::<LitStr>(mac.tokens.clone())
                .map(|literal| literal.value())
                .unwrap_or_else(|_| {
                    self.unresolved_include = Some(mac.tokens.to_string());
                    "<unexpanded>".to_owned()
                });
            if macro_name.as_deref() == Some("include") && include_target != "<unexpanded>" {
                self.code_includes.push(include_target.clone());
            }
            self.record(RustDependencyKind::Include, include_target);
        } else if mac.path.segments.len() > 1
            && mac
                .path
                .segments
                .first()
                .is_some_and(|segment| is_dependency_root(&normalized_ident(&segment.ident)))
        {
            self.record(RustDependencyKind::Macro, target);
        }
        for path in token_stream_paths(&mac.tokens) {
            self.record(RustDependencyKind::Path, path);
        }
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| normalized_ident(&segment.ident))
        .collect::<Vec<_>>()
        .join("::")
}

fn token_stream_paths(tokens: &TokenStream) -> Vec<String> {
    let tokens = tokens.clone().into_iter().collect::<Vec<_>>();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if let TokenTree::Group(group) = &tokens[index] {
            paths.extend(token_stream_paths(&group.stream()));
            index += 1;
            continue;
        }
        let TokenTree::Ident(first) = &tokens[index] else {
            index += 1;
            continue;
        };
        let mut segments = vec![normalized_ident(first)];
        let mut end = index + 1;
        while end + 2 < tokens.len()
            && is_path_separator(&tokens[end..end + 2])
            && matches!(tokens[end + 2], TokenTree::Ident(_))
        {
            if let TokenTree::Ident(ident) = &tokens[end + 2] {
                segments.push(normalized_ident(ident));
            }
            end += 3;
        }
        if segments.len() > 1 && is_dependency_root(&segments[0]) {
            paths.push(segments.join("::"));
            index = end;
        } else {
            index += 1;
        }
    }
    paths
}

fn is_dependency_root(segment: &str) -> bool {
    matches!(
        segment,
        "crate" | "self" | "super" | "std" | "core" | "alloc"
    ) || segment
        .chars()
        .next()
        .is_some_and(|character| character == '_' || character.is_lowercase())
}

fn is_lint_level_attribute(path: &syn::Path) -> bool {
    path.is_ident("allow")
        || path.is_ident("deny")
        || path.is_ident("expect")
        || path.is_ident("forbid")
        || path.is_ident("warn")
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

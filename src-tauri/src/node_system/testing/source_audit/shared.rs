use std::path::{Path, PathBuf};

use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, Lit, Macro, Pat, Token};

pub(super) use crate::test_support::source_audit::{expand_use_tree, is_test_only};

pub(super) fn rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
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

pub(super) fn record(
    offenders: &mut Vec<String>,
    relative: &str,
    line: usize,
    label: &str,
    token: &str,
) {
    offenders.push(format!("{relative}:{line}:{label}:{token}"));
}

pub(super) fn line_for(source: &str, token: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(token))
        .map_or(1, |line| line + 1)
}

pub(super) fn macro_arguments(mac: &Macro) -> Option<Punctuated<Expr, Token![,]>> {
    Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(mac.tokens.clone())
        .ok()
}

pub(super) fn static_string_expression(expr: &Expr) -> Option<String> {
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

pub(super) fn pattern_ident(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Type(typed) => pattern_ident(&typed.pat),
        Pat::Paren(parenthesized) => pattern_ident(&parenthesized.pat),
        _ => None,
    }
}

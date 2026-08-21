use std::path::{Path, PathBuf};

use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, Lit, Macro, Meta, Pat, Token, UseTree};

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

pub(super) fn expand_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
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

pub(super) fn cfg_predicate_is_exclusively_test(meta: &Meta) -> bool {
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

pub(super) fn is_test_only(attributes: &[syn::Attribute]) -> bool {
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

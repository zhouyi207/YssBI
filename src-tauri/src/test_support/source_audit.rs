use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, MetaList, Token, UseTree};

pub(crate) fn normalized_ident(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
}

pub(crate) fn expand_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(normalized_ident(&path.ident));
            expand_use_tree(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            let ident = normalized_ident(&name.ident);
            if ident != "self" || path.is_empty() {
                path.push(ident);
            }
            paths.push(path);
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            let ident = normalized_ident(&rename.ident);
            if ident != "self" || path.is_empty() {
                path.push(ident);
            }
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

fn cfg_predicate_is_exclusively_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("all") || list.path.is_ident("any") => {
            let Some(predicates) = parse_meta_list(list) else {
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

fn parse_meta_list(list: &MetaList) -> Option<Punctuated<Meta, Token![,]>> {
    Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .ok()
}

fn cfg_attr_can_emit_production_path(meta: &Meta) -> bool {
    let Meta::List(list) = meta else {
        return false;
    };
    let Some(arguments) = parse_meta_list(list) else {
        return false;
    };
    let Some(predicate) = arguments.first() else {
        return false;
    };
    if cfg_predicate_is_exclusively_test(predicate) {
        return false;
    }
    arguments.iter().skip(1).any(|attribute| {
        attribute.path().is_ident("path")
            || (attribute.path().is_ident("cfg_attr")
                && cfg_attr_can_emit_production_path(attribute))
    })
}

pub(crate) fn has_production_cfg_attr_path(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg_attr") && cfg_attr_can_emit_production_path(&attribute.meta)
    })
}

pub(crate) fn is_test_only(attributes: &[syn::Attribute]) -> bool {
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
        let Some(predicates) = parse_meta_list(cfg) else {
            return false;
        };
        predicates.len() == 1
            && predicates
                .first()
                .is_some_and(cfg_predicate_is_exclusively_test)
    })
}

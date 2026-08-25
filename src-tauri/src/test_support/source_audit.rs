use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Meta, Token, UseTree};

pub(crate) fn expand_use_tree(
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

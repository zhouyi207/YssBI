//! Searchable catalog text is prepared once alongside each localized projection.
use crate::LocalizedCatalogItem;
use pinyin_pro::{
    options::{NonZh, PatternKind, PinyinOptions, PinyinOutput, ToneType},
    pinyin,
};
use std::collections::BTreeSet;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub fn normalize_catalog_search_text(value: &str) -> Box<str> {
    let mut result = String::with_capacity(value.len());
    let mut separated = true;
    for character in value
        .nfkd()
        .case_fold()
        .filter(|ch| !is_combining_mark(*ch))
    {
        // Preserve the catalog's existing Scandinavian-letter folding.
        let character = if character == 'ø' { 'o' } else { character };
        if character.is_alphanumeric() {
            result.push(character);
            separated = false;
        } else if !separated {
            result.push(' ');
            separated = true;
        }
    }
    result.trim_end().into()
}

/// Titles, protocol identities, aliases and resource names share the same query normalization.
pub fn catalog_search_text(item: &LocalizedCatalogItem) -> Box<str> {
    let sources: BTreeSet<_> = std::iter::once(item.title.as_ref())
        .chain(item.aliases.iter().map(AsRef::as_ref))
        .chain(item.technical_terms.iter().map(AsRef::as_ref))
        .chain(item.backend_search_text.iter().map(AsRef::as_ref))
        .chain(item.resource_names.iter().map(AsRef::as_ref))
        .filter(|source: &&str| !source.is_empty())
        .collect();
    let mut normalized = BTreeSet::from([normalize_catalog_search_text(&item.node_type_id)]);
    for source in sources {
        normalized.insert(normalize_catalog_search_text(source));
        if source.is_ascii() {
            continue;
        }
        // The phrase dictionary retains contextual readings, e.g. 重复 and 重庆.
        // Non-Chinese segments remain intact in mixed resource names.
        for (pattern, separator) in [(PatternKind::Pinyin, " "), (PatternKind::First, "")] {
            let PinyinOutput::Str(phonetic) = pinyin(
                source,
                PinyinOptions {
                    pattern,
                    tone_type: ToneType::None,
                    non_zh: NonZh::Consecutive,
                    separator: separator.into(),
                    ..Default::default()
                },
            ) else {
                unreachable!("string output requested")
            };
            normalized.insert(normalize_catalog_search_text(&phonetic));
        }
    }
    normalized
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_search_covers_metadata_and_contextual_pinyin() {
        let system = crate::build_builtin_node_system().unwrap();
        let catalog = system.catalog.localize(&system.registry, "zh-CN");
        let mut item = catalog.items[0].clone();
        item.node_type_id = "example.repeat_measure".into();
        item.title = "重复测量".into();
        item.aliases = vec!["求和".into()];
        item.technical_terms = vec!["加法术语".into()];
        item.backend_search_text = vec!["backend-add-token".into()];
        item.resource_names = vec!["重庆数据 CAFÉ".into()];
        let document = catalog_search_text(&item);
        for query in [
            "重复测量",
            "qiu he",
            "jfsy",
            "EXAMPLE.REPEAT_MEASURE",
            "backend-add-token",
            "chong fu ce liang",
            "cfcl",
            "chong qing cafe",
            "cqsj",
            "ＣＡＦÉ",
        ] {
            assert!(
                normalize_catalog_search_text(query)
                    .split_whitespace()
                    .all(|term| document.contains(term)),
                "{query}"
            );
        }
        assert_eq!(
            normalize_catalog_search_text("Straße Ø cafe\u{301}"),
            "strasse o cafe".into()
        );
    }
}

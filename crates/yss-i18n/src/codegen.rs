//! Build-time compilation of per-locale JSON message files into a [`StaticCatalog`]
//! expression. A catalog ships one file per supported locale, and every locale must define
//! the same keys, value kinds and `%{name}` placeholders, so an incomplete translation
//! fails the build instead of silently falling back at runtime. A plural message is an
//! object keyed by CLDR categories, and each locale defines exactly the categories of its
//! own plural rule.
//!
//! [`StaticCatalog`]: crate::StaticCatalog
use crate::PluralCategory;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    List(Vec<String>),
    /// Forms in canonical category order, so `other` is last.
    Plural(Vec<(PluralCategory, String)>),
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    locales: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Catalog {
    /// Loads every `<locale>.json` directly inside `directory`. Nested objects join their
    /// keys with `.`; leaves are strings, arrays of strings, or plural messages.
    pub fn load(directory: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        let mut locales = BTreeMap::new();
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let locale = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("{}: locale file name is not UTF-8", path.display()))?
                .to_owned();
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let json = serde_json::from_str::<serde_json::Value>(&text)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if !json.is_object() {
                return Err(format!("{}: the root must be an object", path.display()));
            }
            let mut messages = BTreeMap::new();
            flatten(&path, "", &json, &mut messages)?;
            locales.insert(locale, messages);
        }
        let found = locales.keys().map(String::as_str).collect::<BTreeSet<_>>();
        if found != BTreeSet::from(crate::LOCALES) {
            return Err(format!(
                "{}: locale files must be exactly {:?}, found {found:?}",
                directory.display(),
                crate::LOCALES
            ));
        }
        let catalog = Self { locales };
        catalog.check_parity()?;
        Ok(catalog)
    }

    /// Keys of the catalog; parity guarantees every locale defines the same set.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.locales
            .values()
            .next()
            .into_iter()
            .flat_map(|messages| messages.keys().map(String::as_str))
    }

    /// A Rust expression of type `yss_i18n::StaticCatalog`.
    pub fn to_rust(&self) -> String {
        let mut out = String::from("&[");
        for (locale, messages) in &self.locales {
            out.push_str(&format!("({locale:?}, &["));
            for (key, value) in messages {
                match value {
                    Value::Text(text) => {
                        out.push_str(&format!("({key:?}, yss_i18n::Message::Text({text:?})),"));
                    }
                    Value::List(items) => {
                        out.push_str(&format!("({key:?}, yss_i18n::Message::List(&["));
                        for item in items {
                            out.push_str(&format!("{item:?},"));
                        }
                        out.push_str("])),");
                    }
                    Value::Plural(forms) => {
                        out.push_str(&format!("({key:?}, yss_i18n::Message::Plural(&["));
                        for (category, text) in forms {
                            out.push_str(&format!(
                                "(yss_i18n::PluralCategory::{category:?}, {text:?}),"
                            ));
                        }
                        out.push_str("])),");
                    }
                }
            }
            out.push_str("]),");
        }
        out.push(']');
        out
    }

    fn check_parity(&self) -> Result<(), String> {
        let reference_locale = crate::FALLBACK_LOCALE;
        let Some(reference) = self.locales.get(reference_locale) else {
            return Err(format!("{reference_locale} messages are missing"));
        };
        let mut problems = Vec::new();
        for (locale, messages) in &self.locales {
            if locale != reference_locale {
                for key in reference.keys().filter(|key| !messages.contains_key(*key)) {
                    problems.push(format!("{locale} is missing {key}"));
                }
                for key in messages.keys().filter(|key| !reference.contains_key(*key)) {
                    problems.push(format!(
                        "{locale} defines {key}, which {reference_locale} lacks"
                    ));
                }
            }
            let categories = crate::plural_categories(locale);
            for (key, value) in messages {
                if let Value::Plural(forms) = value
                    && !forms
                        .iter()
                        .map(|(category, _)| *category)
                        .eq(categories.iter().copied())
                {
                    problems.push(format!(
                        "{locale} {key} must define the plural forms {}, found {}",
                        category_names(categories.iter().copied()),
                        category_names(forms.iter().map(|(category, _)| *category))
                    ));
                }
                let Some(expected) = reference.get(key) else {
                    continue;
                };
                if std::mem::discriminant(expected) != std::mem::discriminant(value) {
                    problems.push(format!(
                        "{locale} {key} is not the same kind of value as in {reference_locale}"
                    ));
                    continue;
                }
                let Some(expected) = reference_template(expected) else {
                    continue;
                };
                let expected = placeholders(expected);
                if templates(value)
                    .into_iter()
                    .any(|text| placeholders(text) != expected)
                {
                    problems.push(format!(
                        "{locale} {key} uses different %{{name}} placeholders than {reference_locale}"
                    ));
                }
            }
        }
        if problems.is_empty() {
            return Ok(());
        }
        let shown = problems.len().min(20);
        Err(format!(
            "locale files differ ({} problems):\n{}",
            problems.len(),
            problems[..shown].join("\n")
        ))
    }
}

fn flatten(
    path: &Path,
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    let leaf = match value {
        serde_json::Value::Object(children) => {
            if let Some(forms) = plural_forms(path, prefix, children)? {
                Value::Plural(forms)
            } else {
                for (key, child) in children {
                    let key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    flatten(path, &key, child, out)?;
                }
                return Ok(());
            }
        }
        serde_json::Value::String(text) => Value::Text(text.clone()),
        serde_json::Value::Array(items) => Value::List(
            items
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        format!("{}: {prefix} must contain only strings", path.display())
                    })
                })
                .collect::<Result<_, _>>()?,
        ),
        _ => {
            return Err(format!(
                "{}: {prefix} must be a string or an array of strings",
                path.display()
            ));
        }
    };
    if out.insert(prefix.to_owned(), leaf).is_some() {
        return Err(format!("{}: {prefix} is defined twice", path.display()));
    }
    Ok(())
}

/// An object whose keys are all CLDR category names is one plural message, not a namespace.
fn plural_forms(
    path: &Path,
    prefix: &str,
    children: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Vec<(PluralCategory, String)>>, String> {
    let Some(categories) = children
        .keys()
        .map(|name| {
            PluralCategory::ALL
                .into_iter()
                .find(|category| category.name() == name)
        })
        .collect::<Option<Vec<_>>>()
        .filter(|categories| !categories.is_empty())
    else {
        return Ok(None);
    };
    let mut forms = categories
        .into_iter()
        .zip(children.values())
        .map(|(category, text)| {
            text.as_str()
                .map(|text| (category, text.to_owned()))
                .ok_or_else(|| {
                    format!(
                        "{}: {prefix}.{} must be a string",
                        path.display(),
                        category.name()
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    forms.sort_by_key(|(category, _)| *category);
    if forms.last().map(|(category, _)| *category) != Some(PluralCategory::Other) {
        return Err(format!(
            "{}: the plural message {prefix} must define `other`",
            path.display()
        ));
    }
    Ok(Some(forms))
}

/// The text whose placeholders every translation of a message must use.
fn reference_template(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text),
        Value::List(_) => None,
        Value::Plural(forms) => forms.last().map(|(_, text)| text.as_str()),
    }
}

fn templates(value: &Value) -> Vec<&str> {
    match value {
        Value::Text(text) => vec![text],
        Value::List(_) => Vec::new(),
        Value::Plural(forms) => forms.iter().map(|(_, text)| text.as_str()).collect(),
    }
}

fn category_names(categories: impl Iterator<Item = PluralCategory>) -> String {
    format!(
        "[{}]",
        categories
            .map(PluralCategory::name)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn placeholders(text: &str) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find("%{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            break;
        };
        names.insert(&after[..end]);
        rest = &after[end + 1..];
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(std::path::PathBuf);

    impl Directory {
        fn new(files: &[(&str, &str)]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "yss-i18n-codegen-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            for (name, content) in files {
                std::fs::write(path.join(name), content).unwrap();
            }
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nested_objects_flatten_and_lists_compile_into_one_static_expression() {
        let directory = Directory::new(&[
            (
                "en-US.json",
                r#"{"nodes": {"yssbi.add": {"title": "Add %{n}", "aliases": ["plus", "+"]}}}"#,
            ),
            (
                "zh-CN.json",
                r#"{"nodes": {"yssbi.add": {"title": "加 %{n}", "aliases": ["加法"]}}}"#,
            ),
        ]);
        let catalog = Catalog::load(&directory.0).unwrap();

        assert_eq!(
            catalog.keys().collect::<Vec<_>>(),
            ["nodes.yssbi.add.aliases", "nodes.yssbi.add.title"]
        );
        assert_eq!(
            catalog.to_rust(),
            concat!(
                r#"&[("en-US", &[("nodes.yssbi.add.aliases", yss_i18n::Message::List(&["plus","+",])),"#,
                r#"("nodes.yssbi.add.title", yss_i18n::Message::Text("Add %{n}")),]),"#,
                r#"("zh-CN", &[("nodes.yssbi.add.aliases", yss_i18n::Message::List(&["加法",])),"#,
                r#"("nodes.yssbi.add.title", yss_i18n::Message::Text("加 %{n}")),]),]"#,
            )
        );
    }

    #[test]
    fn locales_must_agree_on_keys_kinds_and_placeholders() {
        for (zh, problem) in [
            (r#"{"a": "甲"}"#, "zh-CN is missing b"),
            (r#"{"a": "甲", "b": "乙", "c": "丙"}"#, "zh-CN defines c"),
            (
                r#"{"a": ["甲"], "b": "乙"}"#,
                "zh-CN a is not the same kind",
            ),
            (r#"{"a": "甲", "b": "%{m}"}"#, "zh-CN b uses different"),
        ] {
            let directory = Directory::new(&[
                ("en-US.json", r#"{"a": "A", "b": "%{n}"}"#),
                ("zh-CN.json", zh),
            ]);
            let error = Catalog::load(&directory.0).unwrap_err();
            assert!(error.contains(problem), "{error}");
        }
    }

    #[test]
    fn plural_messages_compile_with_the_forms_of_each_locale() {
        let directory = Directory::new(&[
            (
                "en-US.json",
                r#"{"rows": {"other": "%{count} rows", "one": "%{count} row"}}"#,
            ),
            ("zh-CN.json", r#"{"rows": {"other": "%{count} 行"}}"#),
        ]);
        let catalog = Catalog::load(&directory.0).unwrap();

        assert_eq!(
            catalog.to_rust(),
            concat!(
                r#"&[("en-US", &[("rows", yss_i18n::Message::Plural(&["#,
                r#"(yss_i18n::PluralCategory::One, "%{count} row"),"#,
                r#"(yss_i18n::PluralCategory::Other, "%{count} rows"),])),]),"#,
                r#"("zh-CN", &[("rows", yss_i18n::Message::Plural(&["#,
                r#"(yss_i18n::PluralCategory::Other, "%{count} 行"),])),]),]"#,
            )
        );
    }

    #[test]
    fn plural_messages_follow_locale_rules_and_share_placeholders() {
        let english = r#"{"rows": {"one": "%{count} row", "other": "%{count} rows"}}"#;
        let chinese = r#"{"rows": {"other": "%{count} 行"}}"#;
        for (en, zh, problem) in [
            (
                english,
                r#"{"rows": {"one": "%{count} 行", "other": "%{count} 行"}}"#,
                "zh-CN rows must define the plural forms [other], found [one, other]",
            ),
            (
                r#"{"rows": {"other": "%{count} rows"}}"#,
                chinese,
                "en-US rows must define the plural forms [one, other], found [other]",
            ),
            (
                r#"{"rows": {"one": "one row", "other": "%{count} rows"}}"#,
                chinese,
                "en-US rows uses different",
            ),
            (
                english,
                r#"{"rows": "%{count} 行"}"#,
                "zh-CN rows is not the same kind",
            ),
            (
                r#"{"rows": {"one": "%{count} row"}}"#,
                chinese,
                "rows must define `other`",
            ),
            (
                r#"{"rows": {"one": 1, "other": "%{count} rows"}}"#,
                chinese,
                "rows.one must be a string",
            ),
        ] {
            let directory = Directory::new(&[("en-US.json", en), ("zh-CN.json", zh)]);
            let error = Catalog::load(&directory.0).unwrap_err();
            assert!(error.contains(problem), "{error}");
        }
    }

    #[test]
    fn catalogs_ship_exactly_the_supported_locales() {
        for files in [
            &[("en-US.json", "{}")][..],
            &[
                ("en-US.json", "{}"),
                ("zh-CN.json", "{}"),
                ("fr-FR.json", "{}"),
            ][..],
        ] {
            let directory = Directory::new(files);
            let error = Catalog::load(&directory.0).unwrap_err();
            assert!(error.contains("locale files must be exactly"), "{error}");
        }
    }

    #[test]
    fn values_must_be_text_or_text_lists() {
        let directory = Directory::new(&[("en-US.json", r#"{"a": {"b": 1}}"#)]);
        let error = Catalog::load(&directory.0).unwrap_err();
        assert!(error.contains("a.b must be a string"), "{error}");
    }
}

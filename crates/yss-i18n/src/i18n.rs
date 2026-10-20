//! Locale resolution, plural rules, message lookup and formatting shared by every localized
//! crate. Each owner keeps its messages in per-locale JSON files that the `codegen` feature
//! compiles into a [`StaticCatalog`] at build time.

#[cfg(feature = "codegen")]
pub mod codegen;

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
pub use runtime::*;

pub const ENGLISH: &str = "en-US";
pub const CHINESE: &str = "zh-CN";

/// Every message catalog ships exactly these locales; the build fails otherwise. A new locale
/// also needs its rule in [`plural_categories`] and [`plural_category`].
pub const LOCALES: [&str; 2] = [ENGLISH, CHINESE];

/// Unsupported locale requests resolve here. It is also the source language of authored
/// node help, whose pages other languages translate.
pub const FALLBACK_LOCALE: &str = ENGLISH;

/// One catalog value: display text, an ordered list such as search aliases, or the forms of
/// a message whose wording depends on a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Text(&'static str),
    List(&'static [&'static str]),
    Plural(&'static [(PluralCategory, &'static str)]),
}

/// Messages compiled from locale files: `(locale, [(key, message)])` with sorted keys.
pub type StaticCatalog = &'static [(&'static str, &'static [(&'static str, Message)])];

/// CLDR cardinal plural categories, in canonical order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub const ALL: [Self; 6] = [
        Self::Zero,
        Self::One,
        Self::Two,
        Self::Few,
        Self::Many,
        Self::Other,
    ];

    /// The CLDR name, which locale files use as the key of each form.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// The categories `locale` distinguishes for whole numbers, in canonical order. Plural
/// messages define exactly these forms.
pub fn plural_categories(locale: &str) -> &'static [PluralCategory] {
    match resolve_locale(locale) {
        ENGLISH => &[PluralCategory::One, PluralCategory::Other],
        // Chinese does not inflect for number.
        _ => &[PluralCategory::Other],
    }
}

/// The category the CLDR rule of `locale` assigns to `count`.
pub fn plural_category(locale: &str, count: usize) -> PluralCategory {
    match resolve_locale(locale) {
        ENGLISH if count == 1 => PluralCategory::One,
        _ => PluralCategory::Other,
    }
}

/// Matches by language subtag, so `zh-Hant-TW` and `ZH_cn` both select `zh-CN`.
pub fn resolve_locale(locale: &str) -> &'static str {
    let requested = locale.trim().split(['-', '_']).next().unwrap_or_default();
    LOCALES
        .into_iter()
        .find(|supported| language(supported).eq_ignore_ascii_case(requested))
        .unwrap_or(FALLBACK_LOCALE)
}

/// The language subtag of a locale, such as `en` for `en-US`.
pub fn language(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::*;

    static CATALOG: StaticCatalog = &[
        (
            "en-US",
            &[
                ("aliases", Message::List(&["hello", "hi"])),
                ("greeting", Message::Text("Hello")),
                (
                    "rows",
                    Message::Plural(&[
                        (PluralCategory::One, "%{count} row"),
                        (PluralCategory::Other, "%{count} rows"),
                    ]),
                ),
            ],
        ),
        (
            "zh-CN",
            &[
                ("aliases", Message::List(&["你好"])),
                ("greeting", Message::Text("你好")),
                (
                    "rows",
                    Message::Plural(&[(PluralCategory::Other, "%{count} 行")]),
                ),
            ],
        ),
    ];

    #[test]
    fn explicit_locales_resolve_by_language_and_unsupported_ones_use_the_fallback() {
        assert!(LOCALES.contains(&FALLBACK_LOCALE));
        std::thread::scope(|scope| {
            for (locale, expected) in [
                (" EN_us ", "Hello"),
                ("ZH_cn", "你好"),
                ("zh-Hant-TW", "你好"),
                ("zhx", "Hello"),
                ("", "Hello"),
            ] {
                scope.spawn(move || {
                    for _ in 0..32 {
                        assert_eq!(translate(CATALOG, locale, "greeting"), expected);
                    }
                });
            }
        });
        assert_eq!(lookup(CATALOG, "zh-CN", "missing.key"), None);
        assert_eq!(translate(CATALOG, "zh-CN", "missing.key"), "missing.key");
        assert_eq!(list(CATALOG, "zh-CN", "aliases"), Some(&["你好"][..]));
        assert_eq!(lookup(CATALOG, "en-US", "aliases"), None);
    }

    #[test]
    fn plural_forms_follow_each_locales_rule() {
        for (locale, count, expected) in [
            ("en-US", 0, "%{count} rows"),
            ("en-US", 1, "%{count} row"),
            ("en-US", 2, "%{count} rows"),
            ("zh-CN", 1, "%{count} 行"),
            ("fr-FR", 1, "%{count} row"),
        ] {
            assert_eq!(plural(CATALOG, locale, "rows", count), Some(expected));
        }
        assert_eq!(plural(CATALOG, "en-US", "greeting", 1), None);
        for locale in LOCALES {
            for count in 0..=1000 {
                assert!(plural_categories(locale).contains(&plural_category(locale, count)));
            }
        }
    }

    #[test]
    fn argument_values_are_literal_even_when_they_contain_other_placeholders() {
        assert_eq!(
            format(
                "%{name}: %{count} / %{unknown}",
                &[("name", "%{count} {{count}} 世界"), ("count", "2")],
            ),
            "%{count} {{count}} 世界: 2 / %{unknown}",
        );
    }
}

use std::borrow::Cow;

pub use rust_i18n::{Backend, SimpleBackend};
use smallvec::SmallVec;

pub fn resolve_locale(locale: &str, fallback: &'static str) -> &'static str {
    let language = locale.trim().split(['-', '_']).next().unwrap_or_default();
    if language.eq_ignore_ascii_case("en") {
        "en-US"
    } else if language.eq_ignore_ascii_case("zh") {
        "zh-CN"
    } else {
        fallback
    }
}

pub fn translate<'a>(
    backend: &'a (impl Backend + ?Sized),
    locale: &str,
    key: &'a str,
    fallback: &'static str,
) -> Cow<'a, str> {
    let locale = resolve_locale(locale, fallback);
    backend
        .translate(locale, key)
        .or_else(|| {
            (locale != fallback)
                .then(|| backend.translate(fallback, key))
                .flatten()
        })
        .unwrap_or(Cow::Borrowed(key))
}

pub fn format<S: AsRef<str>>(template: &str, arguments: &[(&str, S)]) -> String {
    let mut names = SmallVec::<[&str; 8]>::with_capacity(arguments.len());
    let mut values = SmallVec::<[Cow<'_, str>; 8]>::with_capacity(arguments.len());
    for (name, value) in arguments {
        names.push(name);
        values.push(Cow::Borrowed(value.as_ref()));
    }
    rust_i18n::replace_patterns_cow(template, &names, &values)
}

pub fn format_values(template: &str, names: &[&str], values: &[String]) -> String {
    rust_i18n::replace_patterns(template, names, values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn explicit_locales_are_isolated_and_missing_messages_use_the_callers_fallback() {
        let mut backend = SimpleBackend::new();
        backend.add_translations(
            "en-US".into(),
            HashMap::from([
                ("greeting".into(), "Hello".into()),
                ("english".into(), "Only English".into()),
            ]),
        );
        backend.add_translations(
            "zh-CN".into(),
            HashMap::from([
                ("greeting".into(), "你好".into()),
                ("chinese".into(), "仅中文".into()),
            ]),
        );
        std::thread::scope(|scope| {
            for (locale, expected) in [
                (" EN_us ", "Hello"),
                ("ZH_cn", "你好"),
                ("zh-Hant-TW", "你好"),
            ] {
                let backend = &backend;
                scope.spawn(move || {
                    for _ in 0..32 {
                        assert_eq!(translate(backend, locale, "greeting", "en-US"), expected);
                    }
                });
            }
        });
        assert_eq!(
            translate(&backend, "zh-CN", "english", "en-US"),
            "Only English"
        );
        assert_eq!(translate(&backend, "en-US", "chinese", "zh-CN"), "仅中文");
        assert_eq!(translate(&backend, "zhx", "greeting", "en-US"), "Hello");
        assert_eq!(translate(&backend, "unknown", "greeting", "zh-CN"), "你好");
        assert_eq!(
            translate(&backend, "zh-CN", "missing.key", "en-US"),
            "missing.key"
        );
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

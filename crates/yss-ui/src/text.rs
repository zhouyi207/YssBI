//! Desktop localization uses the component library's process locale as its single runtime owner.
use std::borrow::Cow;

pub fn locale() -> &'static str {
    yss_i18n::resolve_locale(&gpui_kit::component::locale())
}

pub fn set_locale(language: &str) {
    let locale = yss_i18n::resolve_locale(language);
    // The component library names its English messages `en`.
    gpui_kit::component::set_locale(if locale == yss_i18n::ENGLISH {
        "en"
    } else {
        locale
    });
}

/// Compiled from `locales/<locale>.json` by the build script.
static MESSAGES: yss_i18n::StaticCatalog = include!(concat!(env!("OUT_DIR"), "/messages.rs"));

pub fn t(key: &str) -> Cow<'_, str> {
    Cow::Borrowed(yss_i18n::translate(MESSAGES, locale(), key))
}

pub fn translate(key: &str) -> String {
    t(key).into_owned()
}

/// For keys derived from runtime codes that may have no message, so callers can show the
/// code itself or their own fallback instead of the key.
pub fn optional(key: &str) -> Option<String> {
    yss_i18n::lookup(MESSAGES, locale(), key).map(str::to_owned)
}

/// Text in an explicit locale, such as each language's own name in the language picker.
pub fn translate_in(locale: &str, key: &str) -> String {
    yss_i18n::translate(MESSAGES, locale, key).to_owned()
}

/// The form of a plural message that the current locale's rule selects for `count`, with
/// `%{count}` filled in.
pub fn plural(key: &str, count: usize) -> String {
    plural_format(key, count, &[("count", count.to_string())])
}

/// Like [`plural`] for messages with more placeholders or a custom display of the number:
/// `count` only selects the form, and `arguments` fill every placeholder.
pub fn plural_format(key: &str, count: usize, arguments: &[(&str, String)]) -> String {
    let template = yss_i18n::plural(MESSAGES, locale(), key, count).unwrap_or(key);
    yss_i18n::format(template, arguments)
}

pub fn input_placeholder(
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    key: &str,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) {
    let placeholder = t(key);
    if input.read(cx).presentation().placeholder().as_ref() != placeholder.as_ref() {
        let placeholder = placeholder.into_owned();
        input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
    }
}

/// Interpolate once: resource names and user input containing %{braces} remain literal.
pub fn format(key: &str, arguments: &[(&str, String)]) -> String {
    yss_i18n::format(&t(key), arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_interpolation_preserves_placeholder_like_user_input() {
        let name = "report %{name} {{name}}".to_owned();
        for language in yss_i18n::LOCALES {
            let template = translate_in(language, "documents.deleteMessage");
            let rendered = yss_i18n::format(&template, &[("name", name.clone())]);
            assert!(rendered.contains(&name));
            assert_eq!(rendered.matches(&name).count(), 1);
        }
    }

    #[test]
    fn bundled_counts_agree_with_their_number() {
        for (locale, count, expected) in [
            ("en-US", 0, "0 rows"),
            ("en-US", 1, "1 row"),
            ("en-US", 2, "2 rows"),
            ("zh-CN", 1, "1 行"),
        ] {
            let template = yss_i18n::plural(MESSAGES, locale, "counts.rows", count);
            let rendered =
                template.map(|text| yss_i18n::format(text, &[("count", count.to_string())]));
            assert_eq!(rendered.as_deref(), Some(expected), "{locale} {count}");
        }
    }
}

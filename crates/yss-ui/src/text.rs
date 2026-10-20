//! Desktop localization uses the component library's process locale as its single runtime owner.
use std::{borrow::Cow, sync::LazyLock};

pub const DEFAULT_LANGUAGE: &str = yss_i18n::CHINESE;

pub fn locale() -> &'static str {
    yss_i18n::resolve_locale(&gpui_kit::component::locale(), DEFAULT_LANGUAGE)
}

pub fn set_locale(language: &str) {
    gpui_kit::component::set_locale(
        if yss_i18n::resolve_locale(language, DEFAULT_LANGUAGE) == yss_i18n::ENGLISH {
            "en"
        } else {
            DEFAULT_LANGUAGE
        },
    );
}

static BACKEND: LazyLock<yss_i18n::SimpleBackend> =
    LazyLock::new(|| include!(concat!(env!("OUT_DIR"), "/desktop_locales.rs")));

pub fn t(key: &str) -> Cow<'_, str> {
    yss_i18n::translate(&*BACKEND, locale(), key, DEFAULT_LANGUAGE)
}

pub fn translate(key: &str) -> String {
    t(key).into_owned()
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
        for language in ["zh-CN", "en-US"] {
            let template = yss_i18n::translate(
                &*BACKEND,
                language,
                "documents.deleteMessage",
                DEFAULT_LANGUAGE,
            );
            let rendered = yss_i18n::format(&template, &[("name", name.clone())]);
            assert!(rendered.contains(&name));
            assert_eq!(rendered.matches(&name).count(), 1);
        }
    }
}

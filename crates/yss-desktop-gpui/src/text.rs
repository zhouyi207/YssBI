//! Desktop localization uses the component library's process locale as its single runtime owner.
use std::{borrow::Cow, sync::LazyLock};
use yss_application::activity_panel::ActivityText;
use yss_graph_editor::projection::EditorDiagnosticModel;

pub const LANGUAGES: [&str; 2] = ["zh-CN", "en-US"];
pub const DEFAULT_LANGUAGE: &str = "zh-CN";

pub fn locale() -> &'static str {
    yss_i18n::resolve_locale(&gpui_kit::component::locale(), DEFAULT_LANGUAGE)
}

pub fn set_locale(language: &str) {
    gpui_kit::component::set_locale(
        if yss_i18n::resolve_locale(language, DEFAULT_LANGUAGE) == "en-US" {
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

pub fn activity_text(text: &ActivityText) -> String {
    match text {
        ActivityText::Literal(value) => value.clone(),
        ActivityText::Key(key) => translate(key),
    }
}

pub fn graph_diagnostic(diagnostic: &EditorDiagnosticModel) -> String {
    let fallback = || {
        let code = diagnostic.code.as_ref();
        if code.len() <= 128
            && code.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.".contains(&byte)
            })
        {
            format("native.text.diagnosticCode", &[("code", code.to_owned())])
        } else {
            translate("native.text.diagnosticFallback")
        }
    };
    yss_graph_diagnostics::render_diagnostic(
        locale(),
        &diagnostic.code,
        &diagnostic.message_key,
        &diagnostic.arguments,
    )
    .unwrap_or_else(fallback)
}

pub fn run_failure_key(code: yss_graph_execution::error::RunFailureCode) -> &'static str {
    use yss_graph_execution::error::RunFailureCode;
    match code {
        RunFailureCode::KernelFailed => "kernelFailed",
        RunFailureCode::KernelNotFound => "kernelNotFound",
        RunFailureCode::InvalidNumericInput => "invalidNumericInput",
        RunFailureCode::ShapeMismatch => "shapeMismatch",
        RunFailureCode::GroupSchemaMismatch => "groupSchemaMismatch",
        RunFailureCode::GroupKeyCollision => "groupKeyCollision",
        RunFailureCode::InvalidParameter => "invalidParameter",
        RunFailureCode::UnalignedSeries => "unalignedSeries",
        RunFailureCode::BudgetExceeded => "budgetExceeded",
        RunFailureCode::InputLayoutMismatch => "inputLayoutMismatch",
        RunFailureCode::OutputContractMismatch => "outputContractMismatch",
        RunFailureCode::ScientificFailure => "scientificFailure",
        RunFailureCode::DivisionByZero => "divisionByZero",
        RunFailureCode::NonFiniteResult => "nonFiniteResult",
        RunFailureCode::DeadlineExceeded => "deadlineExceeded",
        RunFailureCode::ResourceUnavailable => "resourceUnavailable",
        RunFailureCode::InputResultUnavailable => "inputResultUnavailable",
        RunFailureCode::FinalizationFailed => "finalizationFailed",
    }
}

pub fn run_failure(code: yss_graph_execution::error::RunFailureCode) -> String {
    translate(&format!("runFailure.causes.{}", run_failure_key(code)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_interpolation_preserves_placeholder_like_user_input() {
        let name = "report %{name} {{name}}".to_owned();
        for language in LANGUAGES {
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

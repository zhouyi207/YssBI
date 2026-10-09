use std::sync::OnceLock;
use yss_application::activity_panel::ActivityText;
use yss_graph_editor::projection::EditorDiagnosticModel;

pub const DEFAULT_LANGUAGE: &str = "zh-CN";

pub fn locale() -> &'static str {
    if gpui_component::locale().starts_with("en") {
        "en-US"
    } else {
        DEFAULT_LANGUAGE
    }
}

pub fn t(key: &str) -> &str {
    static LOCALE: OnceLock<serde_json::Value> = OnceLock::new();
    let mut value = LOCALE.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/zh-CN.json"))
            .expect("bundled locale is valid JSON")
    });
    for part in key.split('.') {
        let Some(next) = value.get(part) else {
            return key;
        };
        value = next;
    }
    value.as_str().unwrap_or(key)
}

pub fn translate(key: &str) -> String {
    t(key).to_owned()
}

pub fn input_placeholder(
    input: &gpui::Entity<gpui_component::input::InputState>,
    key: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let placeholder = translate(key);
    if input.read(cx).presentation().placeholder().as_ref() != placeholder.as_str() {
        let placeholder = placeholder.to_owned();
        input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
    }
}

/// Interpolate once: resource names and user input containing {{braces}} remain literal.
pub fn format(key: &str, arguments: &[(&str, String)]) -> String {
    let template = translate(key);
    let mut rest = template.as_str();
    let mut result = String::with_capacity(rest.len());
    while let Some(open) = rest.find("{{") {
        result.push_str(&rest[..open]);
        let Some(close) = rest[open + 2..].find("}}") else {
            result.push_str(&rest[open..]);
            return result;
        };
        let end = open + 2 + close;
        let name = &rest[open + 2..end];
        if let Some((_, value)) = arguments.iter().find(|(key, _)| *key == name) {
            result.push_str(value);
        } else {
            result.push_str(&rest[open..end + 2]);
        }
        rest = &rest[end + 2..];
    }
    result.push_str(rest);
    result
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
            format!("图问题（{code}）。")
        } else {
            "图中存在需要检查的问题。".into()
        }
    };
    let Some(definition) = yss_graph_diagnostics::GRAPH_DIAGNOSTIC_DEFINITIONS
        .iter()
        .find(|definition| {
            definition.code == diagnostic.code.as_ref()
                && definition.message_key == diagnostic.message_key.as_ref()
        })
    else {
        return fallback();
    };
    if definition
        .argument_names
        .iter()
        .any(|name| !diagnostic.arguments.contains_key(*name))
    {
        return fallback();
    }
    let Some(template) = definition
        .templates
        .iter()
        .find(|template| template.locale == "zh-CN")
    else {
        return fallback();
    };
    let mut rest = template.text;
    let mut text = String::new();
    while let Some(open) = rest.find('{') {
        text.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}').map(|close| open + close) else {
            return fallback();
        };
        let Some(argument) = diagnostic.arguments.get(&rest[open + 1..close]) else {
            return fallback();
        };
        // Substitute once so a user label containing another placeholder stays literal.
        text.extend(argument.chars().take(512).map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        }));
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    text
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

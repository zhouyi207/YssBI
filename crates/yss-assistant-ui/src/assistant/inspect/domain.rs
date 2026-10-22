//! Web and Python results shown as what they are: sources, pages, scripts and runs.
use gpui_kit::component::{ActiveTheme, link::Link, text::TextView};
use gpui_kit::{AnyElement, App, IntoElement, SharedString, div, prelude::*, rems};
use serde::de::DeserializeOwned;
use serde_json::Value;
use yss_harness_contract::{
    PYTHON_INSPECT_ENVIRONMENT_TOOL, PYTHON_INSTALL_PACKAGES_TOOL, PYTHON_READ_SCRIPT_TOOL,
    PYTHON_RUN_SCRIPT_TOOL, PythonEnvironment, PythonPackagesInstalled, PythonRun, PythonScript,
    WEB_FETCH_TOOL, WEB_SEARCH_TOOL, WebPage, WebSearchResult,
};
use yss_ui::text::{format, plural, t};

pub(super) fn details(tool: &str, output: &Value, cx: &App) -> Option<AnyElement> {
    let payload = output.get("payload")?;
    Some(match tool {
        WEB_SEARCH_TOOL => web_search(&decode::<WebSearchResult>(payload)?, cx),
        WEB_FETCH_TOOL => web_page(&decode::<WebPage>(payload)?, cx),
        PYTHON_RUN_SCRIPT_TOOL => python_run(&decode::<PythonRun>(payload)?, cx),
        PYTHON_READ_SCRIPT_TOOL => {
            let script = decode::<PythonScript>(payload)?;
            code_block("python-script", "python", &script.content)
        }
        PYTHON_INSTALL_PACKAGES_TOOL => {
            let installed = decode::<PythonPackagesInstalled>(payload)?;
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(label(
                    format(
                        "panel.assistantPythonRequirements",
                        &[("value", installed.requirements.join(", "))],
                    ),
                    cx,
                ))
                .child(output_block("python-install-output", &installed.output, cx))
                .into_any_element()
        }
        PYTHON_INSPECT_ENVIRONMENT_TOOL => {
            python_environment(&decode::<PythonEnvironment>(payload)?, cx)
        }
        _ => return None,
    })
}

fn decode<T: DeserializeOwned>(payload: &Value) -> Option<T> {
    serde_json::from_value(payload.clone()).ok()
}

fn label(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .min_w_0()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

fn web_search(result: &WebSearchResult, cx: &App) -> AnyElement {
    if result.results.is_empty() {
        return label(t("panel.assistantWebNoResults"), cx).into_any_element();
    }
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .children(result.results.iter().map(|hit| {
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    Link::new(SharedString::from(format!("web-hit-{}", hit.url)))
                        .min_w_0()
                        .truncate()
                        .href(hit.url.clone())
                        .child(if hit.title.is_empty() {
                            hit.url.clone()
                        } else {
                            hit.title.clone()
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(cx.theme().muted_foreground)
                        .child(hit.url.clone()),
                )
                .child(div().min_w_0().child(hit.snippet.clone()))
        }))
        .into_any_element()
}

fn web_page(page: &WebPage, cx: &App) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            Link::new(SharedString::from(format!("web-page-{}", page.url)))
                .min_w_0()
                .truncate()
                .href(page.url.clone())
                .child(page.title.clone().unwrap_or_else(|| page.url.clone())),
        )
        .child(label(
            format(
                "panel.assistantWebPageRange",
                &[
                    ("start", page.offset.to_string()),
                    ("end", (page.offset + page.text.chars().count()).to_string()),
                    ("total", page.total_characters.to_string()),
                ],
            ),
            cx,
        ))
        .into_any_element()
}

fn python_run(run: &PythonRun, cx: &App) -> AnyElement {
    let status = match (run.timed_out, run.exit_code) {
        (true, _) => t("panel.assistantPythonTimedOut").into_owned(),
        (false, Some(code)) => format("panel.assistantPythonExited", &[("code", code.to_string())]),
        (false, None) => t("panel.assistantPythonStopped").into_owned(),
    };
    let failed = run.timed_out || run.exit_code != Some(0);
    let mut body = div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(if failed {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(status),
        )
        .child(output_block("python-run-output", &run.output, cx));
    if !run.output_files.is_empty() {
        body = body
            .child(label(plural("counts.files", run.output_files.len()), cx))
            .children(run.output_files.iter().map(|file| {
                div()
                    .min_w_0()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(file.path.clone()),
                    )
                    .child(label(byte_size(file.bytes), cx))
            }));
    }
    body.into_any_element()
}

fn python_environment(environment: &PythonEnvironment, cx: &App) -> AnyElement {
    let summary = if environment.available {
        format(
            "panel.assistantPythonEnvironment",
            &[
                ("manager", environment.manager.clone().unwrap_or_default()),
                (
                    "version",
                    environment
                        .python_version
                        .clone()
                        .unwrap_or_else(|| t("panel.assistantPythonVersionPending").into_owned()),
                ),
                (
                    "packages",
                    plural("counts.packages", environment.packages.len()),
                ),
            ],
        )
    } else {
        environment.setup.clone().unwrap_or_default()
    };
    let mut body = div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(label(summary, cx));
    if !environment.scripts.is_empty() {
        body = body.child(
            div()
                .min_w_0()
                .font_family(cx.theme().mono_font_family.clone())
                .child(environment.scripts.join("  ")),
        );
    }
    body.into_any_element()
}

fn output_block(id: &'static str, text: &str, cx: &App) -> impl IntoElement {
    div()
        .id(id)
        .min_w_0()
        .max_h(rems(17.5))
        .overflow_y_scroll()
        .p_2()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .font_family(cx.theme().mono_font_family.clone())
        .whitespace_normal()
        .child(if text.is_empty() {
            t("panel.assistantPythonNoOutput").into_owned()
        } else {
            text.to_owned()
        })
}

fn code_block(id: &'static str, language: &str, code: &str) -> AnyElement {
    let fence = if code.contains("```") { "````" } else { "```" };
    TextView::markdown(id, format!("{fence}{language}\n{code}\n{fence}")).into_any_element()
}

fn byte_size(bytes: u64) -> String {
    match bytes {
        0..1_024 => format!("{bytes} B"),
        1_024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1_024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

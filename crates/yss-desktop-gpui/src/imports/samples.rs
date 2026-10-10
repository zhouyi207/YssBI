//! Sample metadata is a read projection; only identity and version enter the import request.
use super::{ImportDialog, ImportRequest, ImportStage, ImportTask};
use crate::text::translate as t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*, px};
use yss_application::database::samples::SampleDataset;

impl ImportDialog {
    pub(super) fn render_samples(
        &self,
        entries: &[SampleDataset],
        load_failed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let loading = matches!(self.task, Some(ImportTask::Discovery));
        let mut content = div()
            .id("import-sample-list")
            .max_h(px(390.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t("importModal.samples.subtitle")),
            );
        if loading || (!load_failed && entries.is_empty()) {
            content = content.child(
                div()
                    .py_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(if loading {
                        "importModal.samples.loading"
                    } else {
                        "importModal.samples.empty"
                    })),
            );
        }
        if load_failed {
            content = content.child(
                Button::new("sample-retry")
                    .small()
                    .ghost()
                    .label(t("importModal.samples.retry"))
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| view.samples(window, cx))),
            );
        }
        for sample in entries {
            let id = sample.id.clone();
            let version = sample.version;
            let pending = matches!(self.task.as_ref(), Some(ImportTask::ImportSample(current)) if current == &id);
            let name_key = format!("importModal.samples.datasets.{id}.name");
            let name = translated_or(&name_key, &sample.name);
            let description_key = format!("importModal.samples.datasets.{id}.description");
            let description = translated_or(
                &description_key,
                &t("importModal.samples.defaultDescription"),
            );
            let size = if sample.byte_size >= 1_000_000 {
                format!("{:.1} MB", sample.byte_size as f64 / 1_000_000.)
            } else {
                format!("{} KB", sample.byte_size.div_ceil(1_000))
            };
            content = content.child(
                div()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(Icon::new(IconName::Database).size_4())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_sm().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::text::format(
                                        "importModal.samples.dimensions",
                                        &[
                                            ("rows", sample.row_count.to_string()),
                                            ("columns", sample.column_count.to_string()),
                                        ],
                                    ))
                                    .child(size),
                            )
                            .child(
                                div().child(
                                    Button::new(gpui_kit::SharedString::from(id.clone()))
                                        .small()
                                        .label(t(if pending {
                                            "importModal.samples.importing"
                                        } else {
                                            "importModal.samples.import"
                                        }))
                                        .loading(pending)
                                        .disabled(self.busy())
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            let ImportStage::Samples { entries, .. } = &view.stage
                                            else {
                                                return;
                                            };
                                            if !entries.iter().any(|sample| {
                                                sample.id == id && sample.version == version
                                            }) {
                                                return;
                                            }
                                            view.submit(
                                                ImportRequest::Sample {
                                                    id: id.clone(),
                                                    version,
                                                },
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                            ),
                    ),
            );
        }
        content.into_any_element()
    }
}

fn translated_or(key: &str, fallback: &str) -> String {
    let value = t(key);
    if value == key {
        fallback.to_owned()
    } else {
        value
    }
}

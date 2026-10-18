//! Sample metadata is a read projection; only identity and version enter the import request.
use super::{ImportDialog, ImportRequest, ImportStage, ImportTask};
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, button::Button};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*};
use yss_application::database::samples::SampleDataset;
use yss_ui::text::translate as t;

impl ImportDialog {
    pub(super) fn render_samples(
        &self,
        entries: &[SampleDataset],
        load_failed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let loading = matches!(self.task, Some(ImportTask::Discovery));
        let muted = cx.theme().muted_foreground;
        let mut list = div()
            .id("import-sample-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if loading || entries.is_empty() {
            list = list.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_sm()
                    .text_color(muted)
                    .child(t(if loading {
                        "importModal.samples.loading"
                    } else if load_failed {
                        "importModal.samples.errors.sample_resource_unavailable"
                    } else {
                        "importModal.samples.empty"
                    }))
                    .when(load_failed && !loading, |state| {
                        state.child(
                            Button::new("sample-retry")
                                .small()
                                .icon(IconName::RefreshCw)
                                .label(t("importModal.samples.retry"))
                                .disabled(self.busy())
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.samples(window, cx)),
                                ),
                        )
                    }),
            );
        }
        for (index, sample) in entries.iter().enumerate() {
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
            let metadata = format!(
                "{} · {size}",
                yss_ui::text::format(
                    "importModal.samples.dimensions",
                    &[
                        ("rows", sample.row_count.to_string()),
                        ("columns", sample.column_count.to_string()),
                    ],
                )
            );
            list = list.child(
                div()
                    .py_3()
                    .when(index > 0, |row| row.border_t_1())
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::Database)
                            .size_5()
                            .flex_shrink_0()
                            .text_color(muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_sm().child(name))
                            .child(div().text_xs().text_color(muted).child(description))
                            .child(div().text_xs().text_color(muted).child(metadata)),
                    )
                    .child(
                        Button::new(gpui_kit::SharedString::from(format!("sample-{id}")))
                            .small()
                            .outline()
                            .label(t("native.imports.import"))
                            .loading(pending)
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let ImportStage::Samples { entries, .. } = &view.stage else {
                                    return;
                                };
                                if !entries
                                    .iter()
                                    .any(|sample| sample.id == id && sample.version == version)
                                {
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
            );
        }
        let content = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(t("importModal.samples.subtitle")),
            )
            .child(list);
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

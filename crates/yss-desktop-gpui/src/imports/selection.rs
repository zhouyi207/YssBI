//! The same virtual selector serves Excel sheets and local or remote SQL tables.
use super::{ImportDialog, ImportKind, ImportStage, SourceLocation};
use crate::text::translate as t;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};

impl ImportDialog {
    pub(super) fn render_selection(
        &self,
        source: &SourceLocation,
        choices: &[String],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (title, hint) = if matches!(source, SourceLocation::Excel(_)) {
            ("importModal.selectSheet", "importModal.sheetHint")
        } else {
            ("importModal.selectTable", "importModal.tableHint")
        };
        let (label, detail) = source_caption(source);
        let generation = self.generation;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(t(title)))
            .child(
                div()
                    .id("import-source-caption")
                    .min_w_0()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(label)
                    .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx)),
            )
            .child(self.name_field(cx))
            .child(
                uniform_list(
                    "import-source-choices",
                    choices.len(),
                    cx.processor(move |view, range: std::ops::Range<usize>, _, cx| {
                        let ImportStage::Selection { choices, .. } = &view.stage else {
                            return vec![];
                        };
                        range
                            .map(|index| {
                                let name = choices[index].clone();
                                div()
                                    .px_1()
                                    .py_1()
                                    .child(
                                        Button::new(("source-table", index))
                                            .w_full()
                                            .ghost()
                                            .label(name.clone())
                                            .disabled(view.busy())
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.select_table(
                                                    name.clone(),
                                                    generation,
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    )
                                    .into_any_element()
                            })
                            .collect()
                    }),
                )
                .h(px(320.)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(hint)),
            )
            .into_any_element()
    }
}

fn source_caption(source: &SourceLocation) -> (String, String) {
    match source {
        SourceLocation::Excel(path)
        | SourceLocation::Sql {
            kind: ImportKind::Sqlite,
            connection: path,
        } => {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_owned();
            (name, path.clone())
        }
        SourceLocation::Sql { kind, connection } => {
            // User info and query options can hold credentials; render only the server address.
            let server = url::Url::parse(connection)
                .ok()
                .filter(|url| url.host_str().is_some())
                .map(|url| url[url::Position::BeforeHost..url::Position::AfterPort].to_owned());
            let title = match server {
                Some(server) => format!("{} · {server}", kind.label()),
                None => kind.label().to_owned(),
            };
            (title.clone(), title)
        }
    }
}

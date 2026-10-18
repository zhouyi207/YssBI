//! The same virtual selector serves Excel sheets and local or remote SQL tables.
use super::{ImportDialog, ImportKind, ImportStage, SourceLocation};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, div, prelude::*, uniform_list};

impl ImportDialog {
    pub(super) fn render_selection(
        &self,
        source: &SourceLocation,
        choices: &[String],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = if matches!(source, SourceLocation::Excel(_)) {
            IconName::Sheet
        } else {
            IconName::Table
        };
        let (label, detail) = source_caption(source);
        let generation = self.generation;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.name_field())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .id("import-source-caption")
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(label)
                            .tooltip(move |window, cx| {
                                Tooltip::new(detail.clone()).build(window, cx)
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .p_1()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                uniform_list(
                                    "import-source-choices",
                                    choices.len(),
                                    cx.processor(
                                        move |view, range: std::ops::Range<usize>, _, cx| {
                                            let ImportStage::Selection { choices, .. } =
                                                &view.stage
                                            else {
                                                return vec![];
                                            };
                                            choices
                                                .get(range)
                                                .unwrap_or_default()
                                                .iter()
                                                .map(|name| {
                                                    choice_row(
                                                        name.clone(),
                                                        icon,
                                                        generation,
                                                        view.busy(),
                                                        cx,
                                                    )
                                                })
                                                .collect()
                                        },
                                    ),
                                )
                                .size_full(),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn choice_row(
    name: String,
    icon: IconName,
    generation: u64,
    busy: bool,
    cx: &mut Context<ImportDialog>,
) -> AnyElement {
    Button::new(SharedString::from(format!("source-table-{name}")))
        .ghost()
        .w_full()
        .accessibility_label(name.clone())
        .disabled(busy)
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(icon)
                        .size_4()
                        .flex_shrink_0()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(div().min_w_0().truncate().child(name.clone())),
        )
        .on_click(cx.listener(move |view, _, window, cx| {
            view.select_table(name.clone(), generation, window, cx)
        }))
        .into_any_element()
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
                None => kind.label().into_owned(),
            };
            (title.clone(), title)
        }
    }
}

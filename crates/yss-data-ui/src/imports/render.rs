use super::{ImportDialog, ImportKind, ImportStage, ImportTask, SourceLocation};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Enter, Input},
};
use gpui_kit::{
    AnyElement, Context, ElementId, IntoElement, Render, SharedString, Window, div, prelude::*,
    rems,
};
use yss_ui::inspector::PropertyField;
use yss_ui::text::t;

impl Render for ImportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        yss_ui::text::input_placeholder(&self.name, "native.imports.namePlaceholder", window, cx);

        let content = match &self.stage {
            ImportStage::Sources => self.render_sources(cx),
            ImportStage::File(kind, path) => self.render_file(*kind, path.clone(), cx),
            ImportStage::Connection(_) => self.render_connection(cx),
            ImportStage::Selection { source, choices } => {
                self.render_selection(source, choices, cx)
            }
            ImportStage::Samples {
                entries,
                load_failed,
            } => self.render_samples(entries, *load_failed, cx),
        };
        div()
            .on_action(
                cx.listener(|view, action: &Enter, window, cx| {
                    view.submit_input(action, window, cx)
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .children(self.render_header(cx))
            .child(div().flex_1().min_h_0().flex().flex_col().child(content))
            .child(self.render_footer(cx))
    }
}
impl ImportDialog {
    fn render_header(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let title = match &self.stage {
            ImportStage::Sources => return None,
            ImportStage::File(kind, _) | ImportStage::Connection(kind) => kind.label(),
            ImportStage::Selection {
                source: SourceLocation::Excel(_),
                ..
            } => t("importModal.selectSheet"),
            ImportStage::Selection { .. } => t("importModal.selectTable"),
            ImportStage::Samples { .. } => t("importModal.categories.samples"),
        };
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("import-back")
                        .small()
                        .ghost()
                        .icon(IconName::ChevronLeft)
                        .tooltip(t("native.imports.back"))
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, _, cx| view.back(cx))),
                )
                .child(div().text_base().child(title))
                .into_any_element(),
        )
    }
    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let status = match self.task.as_ref() {
            Some(ImportTask::Picker) => t("native.imports.choosingFile"),
            Some(ImportTask::Discovery) => t("native.imports.readingSource"),
            Some(ImportTask::ImportSample(_)) => t("importModal.samples.importing"),
            Some(ImportTask::Import) => t("dataOperation.importing"),
            None => "".into(),
        };
        let primary = match &self.stage {
            ImportStage::File(ImportKind::Csv | ImportKind::Parquet, _) => {
                Some(t("native.imports.import"))
            }
            ImportStage::File(..) => Some(t("native.imports.readSource")),
            ImportStage::Connection(_) => Some(t("native.imports.connectAndRead")),
            _ => None,
        };
        let waiting = matches!(self.task, Some(ImportTask::Discovery | ImportTask::Import));
        div()
            .flex()
            .items_center()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(status),
            )
            .child(
                Button::new("import-cancel")
                    .label(t("common.cancel"))
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.cancel(window, cx) {
                            yss_ui::modal_window::close(window, cx);
                        }
                    })),
            )
            .children(primary.map(|label| {
                Button::new("import-submit")
                    .primary()
                    .label(label)
                    .loading(waiting)
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| match view.stage {
                        ImportStage::File(..) => view.submit_file(cx, window),
                        ImportStage::Connection(_) => view.connect(window, cx),
                        _ => {}
                    }))
            }))
            .into_any_element()
    }
    fn render_sources(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy();
        let kind_tile = |kind: ImportKind, cx: &mut Context<Self>| {
            source_tile(
                ("import-source", kind as usize).into(),
                kind.icon(),
                kind.label().into_owned(),
                kind.description().into_owned(),
                cx,
            )
            .disabled(busy)
            .on_click(cx.listener(move |view, _, window, cx| view.choose_kind(kind, window, cx)))
        };
        let section = |title: std::borrow::Cow<'static, str>, cx: &mut Context<Self>| {
            div().flex().flex_col().gap_2().child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(title),
            )
        };
        let grid = || div().grid().grid_cols(2).gap_2();
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                section(t("native.imports.localFile"), cx).child(
                    grid()
                        .child(kind_tile(ImportKind::Csv, cx))
                        .child(kind_tile(ImportKind::Parquet, cx))
                        .child(kind_tile(ImportKind::Excel, cx)),
                ),
            )
            .child(
                section(t("native.imports.databaseConnection"), cx).child(
                    grid()
                        .child(kind_tile(ImportKind::Sqlite, cx))
                        .child(kind_tile(ImportKind::Postgres, cx))
                        .child(kind_tile(ImportKind::Mysql, cx))
                        .child(kind_tile(ImportKind::Mariadb, cx)),
                ),
            )
            .child(
                section(t("importModal.categories.samples"), cx).child(
                    grid().child(
                        source_tile(
                            "import-source-samples".into(),
                            IconName::FlaskConical,
                            t("importModal.categories.samples").into_owned(),
                            t("importModal.samples.defaultDescription").into_owned(),
                            cx,
                        )
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, window, cx| view.samples(window, cx))),
                    ),
                ),
            )
            .into_any_element()
    }
    pub(super) fn name_field(&self) -> PropertyField {
        PropertyField::new(t("native.imports.projectName"))
            .control(Input::new(&self.name).disabled(self.busy()))
    }
    fn render_file(&self, kind: ImportKind, path: String, cx: &mut Context<Self>) -> AnyElement {
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_owned();
        let mut form = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        Icon::new(kind.icon())
                            .size_5()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_sm().truncate().child(file_name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(path),
                            ),
                    )
                    .child(
                        Button::new("import-change-file")
                            .small()
                            .ghost()
                            .label(t("native.imports.chooseAnotherFile"))
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.choose_file(kind, window, cx)
                            })),
                    ),
            )
            .child(self.name_field());
        if kind == ImportKind::Csv {
            form = form
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div().flex_1().min_w_0().child(
                                PropertyField::new(t("native.imports.delimiter"))
                                    .control(Input::new(&self.delimiter).disabled(self.busy())),
                            ),
                        )
                        .child(
                            div().flex_1().min_w_0().child(
                                PropertyField::new(t("native.imports.inferenceRows"))
                                    .control(Input::new(&self.infer_rows).disabled(self.busy())),
                            ),
                        ),
                )
                .child(
                    Checkbox::new("csv-header")
                        .label(SharedString::from(t("native.imports.firstRowHeader")))
                        .checked(self.has_header)
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, checked: &bool, _, cx| {
                            view.has_header = *checked;
                            cx.notify();
                        })),
                );
        }
        form.into_any_element()
    }
    fn render_connection(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy();
        let field = |label: &str, input| {
            div()
                .flex_1()
                .min_w_0()
                .child(PropertyField::new(t(label)).control(Input::new(input).disabled(busy)))
        };
        let mut form = div().flex().flex_col().gap_4().child(
            Checkbox::new("sql-raw")
                .label(SharedString::from(t("native.imports.useConnectionString")))
                .checked(self.connection.raw)
                .disabled(busy)
                .on_click(cx.listener(|view, checked: &bool, _, cx| {
                    view.connection.raw = *checked;
                    cx.notify();
                })),
        );
        if self.connection.raw {
            form = form.child(
                Input::new(&self.connection.raw_url)
                    .mask_toggle()
                    .disabled(busy),
            );
        } else {
            form = form
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(field("importModal.host", &self.connection.host))
                        .child(
                            div()
                                .w(rems(8.))
                                .flex_shrink_0()
                                .child(field("importModal.port", &self.connection.port)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(field("importModal.username", &self.connection.user))
                        .child(field("importModal.password", &self.connection.password)),
                )
                .child(field("importModal.database", &self.connection.database));
        }
        form.child(self.name_field()).into_any_element()
    }
}

/// A source choice: the icon and name identify the format, the description says what it reads.
fn source_tile(
    id: ElementId,
    icon: IconName,
    title: String,
    description: String,
    cx: &Context<ImportDialog>,
) -> Button {
    Button::new(id)
        .outline()
        .w_full()
        .h(rems(3.75))
        .px_3()
        .accessibility_label(title.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    Icon::new(icon)
                        .size_5()
                        .flex_shrink_0()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(div().text_sm().truncate().child(title))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(description),
                        ),
                ),
        )
}

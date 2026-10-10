use super::{ImportDialog, ImportKind, ImportStage, ImportTask};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Enter, Input},
};
use gpui_kit_assets::IconName;

impl Render for ImportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::text::input_placeholder(&self.name, "native.imports.namePlaceholder", window, cx);
        crate::text::input_placeholder(&self.connection.user, "importModal.username", window, cx);
        crate::text::input_placeholder(
            &self.connection.database,
            "native.imports.databaseName",
            window,
            cx,
        );

        let busy = self.busy();
        let compact = f32::from(window.viewport_size().width) <= 720.;
        let mut content = match &self.stage {
            ImportStage::Sources => self.render_sources(cx),
            ImportStage::File(kind, path) => self.render_file(*kind, path.clone(), cx),
            ImportStage::Connection(kind) => self.render_connection(*kind, cx),
            ImportStage::Selection { source, choices } => {
                self.render_selection(source, choices, cx)
            }
            ImportStage::Samples {
                entries,
                load_failed,
            } => self.render_samples(entries, *load_failed, cx),
        };
        if !matches!(self.stage, ImportStage::Sources) {
            content = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Button::new("import-back")
                        .small()
                        .ghost()
                        .icon(IconName::ChevronLeft)
                        .label(crate::text::t("native.imports.back"))
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, _, cx| view.back(cx))),
                )
                .child(content)
                .into_any_element();
        }
        div()
            .on_action(
                cx.listener(|view, action: &Enter, window, cx| {
                    view.submit_input(action, window, cx)
                }),
            )
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.imports.importHint")),
            )
            .child(
                div()
                    .flex()
                    .when(compact, |view| view.flex_col())
                    .gap_4()
                    .min_h(px(350.))
                    .child(self.render_navigation(compact, cx))
                    .child(div().flex_1().min_w_0().child(content)),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_xs().text_color(cx.theme().danger).child(error))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match self.task.as_ref() {
                                Some(ImportTask::Picker) => {
                                    crate::text::t("native.imports.choosingFile")
                                }
                                Some(ImportTask::Discovery) => {
                                    crate::text::t("native.imports.readingSource")
                                }
                                Some(ImportTask::ImportSample(_)) => {
                                    crate::text::t("importModal.samples.importing")
                                }
                                Some(ImportTask::Import) => {
                                    crate::text::t("dataOperation.importing")
                                }
                                None => "",
                            }),
                    )
                    .child(
                        Button::new("import-cancel")
                            .small()
                            .ghost()
                            .label(crate::text::t("common.cancel"))
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.cancel(window, cx) {
                                    crate::modal_window::close(window, cx);
                                }
                            })),
                    ),
            )
    }
}
impl ImportDialog {
    fn render_navigation(&self, compact: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut navigation = div()
            .flex_shrink_0()
            .flex()
            .gap_2()
            .when(compact, |view| {
                view.w_full().flex_wrap().pb_3().border_b_1()
            })
            .when(!compact, |view| {
                view.w(px(150.)).flex_col().pr_3().border_r_1()
            })
            .border_color(cx.theme().border);
        for (index, label) in [
            crate::text::t("native.imports.localFile"),
            crate::text::t("native.imports.databaseConnection"),
            crate::text::t("importModal.categories.samples"),
        ]
        .into_iter()
        .enumerate()
        {
            navigation = navigation.child(
                Button::new(("import-category", index))
                    .small()
                    .ghost()
                    .label(label)
                    .selected(index == self.category)
                    .disabled(self.busy())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.busy() {
                            return;
                        }
                        view.category = index;
                        view.error = None;
                        view.stage = ImportStage::Sources;
                        if index == 2 {
                            view.samples(window, cx);
                        }
                        cx.notify();
                    })),
            );
        }
        navigation.into_any_element()
    }
    fn render_sources(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let kinds: &[ImportKind] = if self.category == 1 {
            &[
                ImportKind::Sqlite,
                ImportKind::Postgres,
                ImportKind::Mysql,
                ImportKind::Mariadb,
            ]
        } else {
            &[ImportKind::Csv, ImportKind::Parquet, ImportKind::Excel]
        };
        let mut view = div().flex().flex_col().gap_3();
        for kind in kinds {
            let kind = *kind;
            let description = match kind {
                ImportKind::Csv => crate::text::t("native.imports.csvDescription"),
                ImportKind::Parquet => crate::text::t("native.imports.parquetDescription"),
                ImportKind::Excel => crate::text::t("native.imports.excelDescription"),
                ImportKind::Sqlite => crate::text::t("native.imports.sqliteDescription"),
                _ => crate::text::t("native.imports.sqlDescription"),
            };
            view = view.child(
                div()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new(kind.label())
                            .ghost()
                            .label(kind.label())
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.choose_kind(kind, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    ),
            );
        }
        view.into_any_element()
    }
    pub(super) fn name_field(&self, cx: &Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.imports.projectName")),
            )
            .child(Input::new(&self.name).disabled(self.busy()))
            .into_any_element()
    }
    fn render_file(
        &self,
        kind: ImportKind,
        path: String,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut view = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(kind.label()))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(path),
            )
            .child(
                Button::new("import-change-file")
                    .small()
                    .ghost()
                    .label(crate::text::t("native.imports.chooseAnotherFile"))
                    .disabled(self.busy())
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.choose_file(kind, window, cx)),
                    ),
            )
            .child(self.name_field(cx));
        if kind == ImportKind::Csv {
            view = view
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .child(crate::text::t("native.imports.delimiter")),
                                )
                                .child(Input::new(&self.delimiter).disabled(self.busy())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .child(crate::text::t("native.imports.inferenceRows")),
                                )
                                .child(Input::new(&self.infer_rows).disabled(self.busy())),
                        ),
                )
                .child(
                    Checkbox::new("csv-header")
                        .label(crate::text::t("native.imports.firstRowHeader"))
                        .checked(self.has_header)
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, checked: &bool, _, cx| {
                            view.has_header = *checked;
                            cx.notify();
                        })),
                );
        }
        view.child(
            Button::new("import-file")
                .label(if matches!(kind, ImportKind::Csv | ImportKind::Parquet) {
                    crate::text::t("native.imports.import")
                } else {
                    crate::text::t("native.imports.readSource")
                })
                .disabled(self.busy())
                .on_click(cx.listener(|view, _, window, cx| view.submit_file(cx, window))),
        )
        .into_any_element()
    }
    fn render_connection(&self, kind: ImportKind, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut view = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().child(kind.label()))
            .child(
                Checkbox::new("sql-raw")
                    .label(crate::text::t("native.imports.useConnectionString"))
                    .checked(self.connection.raw)
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, checked: &bool, _, cx| {
                        view.connection.raw = *checked;
                        cx.notify();
                    })),
            );
        if self.connection.raw {
            view = view.child(
                Input::new(&self.connection.raw_url)
                    .mask_toggle()
                    .disabled(self.busy()),
            );
        } else {
            for (label, input) in [
                (crate::text::t("importModal.host"), &self.connection.host),
                (crate::text::t("importModal.port"), &self.connection.port),
                (
                    crate::text::t("importModal.username"),
                    &self.connection.user,
                ),
                (
                    crate::text::t("importModal.password"),
                    &self.connection.password,
                ),
                (
                    crate::text::t("importModal.database"),
                    &self.connection.database,
                ),
            ] {
                view = view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(58.)).text_xs().child(label))
                        .child(Input::new(input).disabled(self.busy())),
                );
            }
        }
        view.child(self.name_field(cx))
            .child(
                Button::new("sql-connect")
                    .label(crate::text::t("native.imports.connectAndRead"))
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| view.connect(window, cx))),
            )
            .into_any_element()
    }
}

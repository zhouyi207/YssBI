use super::{ImportDialog, ImportKind, ImportStage, ImportTask};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Enter, Input},
};

impl Render for ImportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        .label("返回")
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
                    .child("导入后，数据作为独立资源保存在当前项目中。"),
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
                                Some(ImportTask::Picker) => "正在选择文件…".to_owned(),
                                Some(ImportTask::Discovery) => "正在读取来源…".to_owned(),
                                Some(ImportTask::Import) => "正在导入数据…".to_owned(),
                                Some(ImportTask::ImportSample(_)) => {
                                    crate::text::translate("importModal.samples.importing")
                                }
                                None => String::new(),
                            }),
                    )
                    .child(
                        Button::new("import-cancel")
                            .small()
                            .ghost()
                            .label("取消")
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.cancel(window, cx) {
                                    window.close_dialog(cx);
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
        for (index, label) in ["本地文件", "数据库连接", "示例数据"]
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
                ImportKind::Csv => "文本表格，可设置分隔符、表头和类型推断行数。",
                ImportKind::Parquet => "导入 Parquet 文件，保留来源的字段类型。",
                ImportKind::Excel => "读取工作簿并选择需要导入的工作表。",
                ImportKind::Sqlite => "选择本地 SQLite 文件，再选择数据表。",
                _ => "填写连接配置或连接字符串，再选择数据表。",
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
                    .child("项目中的名称"),
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
                    .label("选择其他文件…")
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
                                .child(div().text_xs().child("分隔符"))
                                .child(Input::new(&self.delimiter).disabled(self.busy())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(div().text_xs().child("类型推断行数"))
                                .child(Input::new(&self.infer_rows).disabled(self.busy())),
                        ),
                )
                .child(
                    Checkbox::new("csv-header")
                        .label("首行包含列名")
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
                    "导入"
                } else {
                    "读取来源"
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
                    .label("使用连接字符串")
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
                ("主机", &self.connection.host),
                ("端口", &self.connection.port),
                ("用户名", &self.connection.user),
                ("密码", &self.connection.password),
                ("数据库", &self.connection.database),
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
                    .label("连接并读取数据表")
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| view.connect(window, cx))),
            )
            .into_any_element()
    }
}

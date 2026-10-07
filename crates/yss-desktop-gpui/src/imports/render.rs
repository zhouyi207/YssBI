use super::{ImportDialog, ImportKind, ImportRequest, ImportStage, ImportTask, SourceLocation};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, uniform_list};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
};

impl Render for ImportDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let mut content = match &self.stage {
            ImportStage::Sources => self.render_sources(cx),
            ImportStage::File(kind, path) => self.render_file(*kind, path.clone(), cx),
            ImportStage::Connection(kind) => self.render_connection(*kind, cx),
            ImportStage::Selection { source, choices } => {
                let title = match source {
                    SourceLocation::Excel(_) => "选择工作表",
                    SourceLocation::Sql { .. } => "选择数据表",
                };
                let count = choices.len();
                let generation = self.generation;
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_sm().child(format!("{title} · {count} 项")))
                    .child(self.name_field(cx))
                    .child(
                        uniform_list(
                            "import-source-choices",
                            count,
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
                                                    .on_click(cx.listener(
                                                        move |view, _, window, cx| {
                                                            view.select_table(
                                                                name.clone(),
                                                                generation,
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    )),
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
                            .child("选择后将复制数据到项目；来源保持原状。"),
                    )
                    .into_any_element()
            }
            ImportStage::Samples(samples) => {
                let mut view = div()
                    .id("import-sample-list")
                    .max_h(px(390.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3();
                for sample in samples {
                    let sample = sample.clone();
                    view = view.child(
                        div()
                            .py_3()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .child(div().text_sm().child(sample.name.clone()))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} 行 · {} 列 · {:.1} MB",
                                                sample.row_count,
                                                sample.column_count,
                                                sample.byte_size as f64 / 1_000_000.
                                            )),
                                    ),
                            )
                            .child(
                                Button::new(gpui::SharedString::from(sample.id.clone()))
                                    .small()
                                    .label("导入")
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.submit(
                                            ImportRequest::Sample {
                                                id: sample.id.clone(),
                                                version: sample.version,
                                            },
                                            window,
                                            cx,
                                        )
                                    })),
                            ),
                    );
                }
                view.child(
                    Button::new("sample-retry")
                        .small()
                        .ghost()
                        .label("重新读取示例")
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, window, cx| view.samples(window, cx))),
                )
                .into_any_element()
            }
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
                    .gap_4()
                    .min_h(px(350.))
                    .child(self.render_navigation(cx))
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
                            .child(match self.task {
                                Some(ImportTask::Picker) => "正在选择文件…",
                                Some(ImportTask::Discovery) => "正在读取来源…",
                                Some(ImportTask::Import) => "正在导入数据…",
                                None => "",
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
    fn render_navigation(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut navigation = div()
            .w(px(150.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .pr_3()
            .border_r_1()
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
    fn name_field(&self, cx: &Context<Self>) -> gpui::AnyElement {
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

use super::DatabaseEditor;
use gpui::{Context, IntoElement, PromptLevel, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    table::TableSelection,
};
use yss_application::database::DatabaseMutation;
use yss_database_schema::DatabaseColumnFact;

const PHYSICAL_TYPES: [&str; 19] = [
    "Bool",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "Float32",
    "Float64",
    "Utf8",
    "Date",
    "Datetime(s)",
    "Datetime(ms)",
    "Datetime(us)",
    "Datetime(ns)",
    "Time",
    "Dictionary(Int32, Utf8)",
];
impl DatabaseEditor {
    pub fn render_details(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut view = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("数据"),
            );
        let Some(meta) = &self.meta else {
            return view
                .child(div().text_xs().child("正在读取元数据…"))
                .into_any_element();
        };
        view = view.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} 行 · {} 列", meta.row_count, meta.column_count)),
        );
        if self.dirty() {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("列设置已写入项目。保存会建立检查点，关闭不会还原已应用的设置。"),
            );
        }
        let selected = self.grid.read(cx).selection();
        let selection = &self.grid.read(cx).delegate().selection;
        let all = selection.all;
        let single_column = selection
            .bounds
            .as_ref()
            .is_some_and(|bounds| bounds.columns.len() == 1)
            && !all;
        if let Some(bounds) = &selection.bounds
            && (bounds.cell_count() > 1 || all)
        {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "已选 {} 行 × {} 列 · {} 个单元格；Ctrl/Cmd+C 复制当前页选区。",
                        bounds.rows.len(),
                        bounds.columns.len(),
                        bounds.cell_count()
                    )),
            );
        }
        if let TableSelection::Cell(row, column) = selected
            && !all
        {
            let table = self.grid.read(cx);
            let page = table.delegate();
            let title = page
                .rows
                .columns()
                .get(column)
                .map(|column| column.name().as_str())
                .unwrap_or("");
            let value = page.text(row, column);
            view = view
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("第 {} 行 · {}", page.offset + row + 1, title)),
                )
                .child(div().text_sm().child(value));
        }
        let column = match selected {
            TableSelection::Cell(_, column) | TableSelection::Column(column) if single_column => {
                Some(column)
            }
            _ => None,
        };
        if let Some(column) = column.and_then(|index| meta.columns.get(index)).cloned() {
            view = view.child(self.column_settings(column, cx));
        } else {
            for column in meta.columns.iter().take(100) {
                view = view.child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(column.name().as_str().to_owned())
                        .child(column.display_type().to_owned()),
                );
            }
        }
        view.into_any_element()
    }
    fn column_settings(
        &self,
        column: DatabaseColumnFact,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let owner = cx.entity().downgrade();
        let semantic_owner = owner.clone();
        let revision = self.revision;
        let name = column.name().as_str().to_owned();
        let current = column.physical_type().to_owned();
        let physical_name = name.clone();
        let supported = column.supported_semantic_types().to_vec();
        let semantic = column
            .semantic()
            .map(|semantic| semantic.kind.as_str())
            .unwrap_or("未设置");
        div()
            .flex()
            .flex_col()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .pt_3()
            .child(div().text_sm().child(name))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("存储类型"),
            )
            .child(
                Button::new("column-physical")
                    .small()
                    .ghost()
                    .label(current.clone())
                    .disabled(self.busy() || !self.ready)
                    .dropdown_menu(move |mut menu, _, _| {
                        for dtype in PHYSICAL_TYPES {
                            let owner = owner.clone();
                            let name = physical_name.clone();
                            let before = current.clone();
                            menu = menu.item(PopupMenuItem::new(dtype).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.confirm_physical(
                                            name.clone(),
                                            before.clone(),
                                            dtype.into(),
                                            revision,
                                            window,
                                            cx,
                                        )
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("数据语义"),
            )
            .child(
                Button::new("column-semantic")
                    .small()
                    .ghost()
                    .label(semantic)
                    .disabled(self.busy() || !self.ready || supported.is_empty())
                    .dropdown_menu(move |mut menu, _, _| {
                        for kind in &supported {
                            let kind = *kind;
                            let owner = semantic_owner.clone();
                            let column = column.clone();
                            menu = menu.item(PopupMenuItem::new(kind.as_str()).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.revision == revision && view.ready && !view.busy() {
                                            view.semantic_dialog(column.clone(), kind, window, cx);
                                        }
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .into_any_element()
    }
    fn confirm_physical(
        &mut self,
        column: String,
        before: String,
        dtype: String,
        revision: yss_project_identity::ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || !self.ready || self.revision != revision || before == dtype {
            return;
        }
        let message = format!("将“{column}”从 {before} 转换为 {dtype}。无法转换的值会使操作失败。");
        let prompt = window.prompt(
            PromptLevel::Warning,
            "转换列类型",
            Some(&message),
            &["转换", "取消"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(0) = prompt.await {
                let _ = view.update_in(cx, |view, window, cx| {
                    view.mutate(
                        DatabaseMutation::CastColumn {
                            column,
                            dtype,
                            force: false,
                        },
                        revision,
                        window,
                        cx,
                    )
                });
            }
        })
        .detach();
    }
}

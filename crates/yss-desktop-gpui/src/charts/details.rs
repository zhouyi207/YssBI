use super::ChartEditor;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use yss_chart_document::{ChartEncodings, ChartType};
use yss_data_contract::{SemanticType, ValueType};
use yss_database_schema::DatabaseColumnFact;

pub(super) fn chart_type_label(kind: ChartType) -> &'static str {
    match kind {
        ChartType::Histogram => "直方图",
        ChartType::Scatter => "散点图",
        ChartType::Line => "折线图",
    }
}
fn numeric(column: &DatabaseColumnFact) -> bool {
    matches!(
        column.data_type(),
        ValueType::Scalar(SemanticType::Numeric | SemanticType::Datetime)
    )
}
impl ChartEditor {
    pub(crate) fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let epoch = self.draft_epoch;
        let publication = self.catalog.publication_revision;
        let owner = cx.entity().downgrade();
        let datasets = self
            .catalog
            .databases
            .iter()
            .map(|entry| {
                (
                    entry.id.clone(),
                    entry.name.clone().unwrap_or_else(|| "未命名数据集".into()),
                )
            })
            .collect::<Vec<_>>();
        let current_name = if self.draft.database_id.is_empty() {
            "选择数据集".into()
        } else {
            datasets
                .iter()
                .find(|(id, _)| *id == self.draft.database_id)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| "已移除的数据集".into())
        };
        let type_owner = owner.clone();
        let mut view = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.path.display_name().as_str().to_owned()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("图表配置"),
            )
            .child(div().text_xs().child("数据集"))
            .child(
                Button::new("chart-dataset")
                    .small()
                    .ghost()
                    .label(current_name)
                    .disabled(self.busy() || !self.available)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (id, name) in std::iter::once((String::new(), "不选择数据集".into()))
                            .chain(datasets.clone())
                        {
                            let owner = owner.clone();
                            menu = menu.item(PopupMenuItem::new(name).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.update_draft(
                                            epoch,
                                            publication,
                                            |document| {
                                                document.database_id = id.clone();
                                                document.encodings =
                                                    ChartEncodings { x: None, y: None };
                                            },
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
            .child(div().text_xs().child("图表类型"))
            .child(
                Button::new("chart-kind")
                    .small()
                    .ghost()
                    .label(chart_type_label(self.draft.chart_type))
                    .disabled(self.busy() || !self.available)
                    .dropdown_menu(move |mut menu, _, _| {
                        for kind in [ChartType::Histogram, ChartType::Scatter, ChartType::Line] {
                            let owner = type_owner.clone();
                            menu = menu.item(PopupMenuItem::new(chart_type_label(kind)).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.update_draft(
                                            epoch,
                                            publication,
                                            |document| {
                                                document.chart_type = kind;
                                                document.encodings =
                                                    ChartEncodings { x: None, y: None };
                                            },
                                            window,
                                            cx,
                                        )
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            );
        if self.draft.chart_type != ChartType::Histogram {
            view = view.child(self.axis_field(true, cx));
        }
        view = view.child(self.axis_field(false, cx));
        if let Some(meta) = &self.meta {
            view = view.child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} 行 · {} 列", meta.row_count, meta.column_count)),
            );
            for column in &meta.columns {
                view = view.child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .child(column.name().as_str().to_owned()),
                        )
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(column.display_type().to_owned()),
                        ),
                );
            }
        }
        view = view.child(
            div()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if self.external_change {
                    "文件已变化，本地配置已保留。保存会写入当前配置。"
                } else if self.dirty() {
                    "配置有未保存的更改。Ctrl/Cmd+S 保存。"
                } else {
                    "已保存"
                }),
        );
        view.into_any_element()
    }
    fn axis_field(&self, x_axis: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let epoch = self.draft_epoch;
        let publication = self.catalog.publication_revision;
        let owner = cx.entity().downgrade();
        let histogram = self.draft.chart_type == ChartType::Histogram;
        let columns = self
            .meta
            .as_ref()
            .map(|meta| {
                meta.columns
                    .iter()
                    .filter(|column| histogram || numeric(column))
                    .map(|column| column.name().as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected = if x_axis {
            self.draft.encodings.x.as_deref()
        } else {
            self.draft.encodings.y.as_deref().or(if histogram {
                self.draft.encodings.x.as_deref()
            } else {
                None
            })
        };
        let label = selected
            .map(|name| {
                if columns.iter().any(|column| column == name) {
                    name.to_owned()
                } else {
                    format!("{name}（当前不可用）")
                }
            })
            .unwrap_or_else(|| "选择列".into());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xs().child(if histogram {
                "分布列"
            } else if x_axis {
                "X 轴"
            } else {
                "Y 轴"
            }))
            .child(
                Button::new(if x_axis { "chart-x" } else { "chart-y" })
                    .small()
                    .ghost()
                    .label(label)
                    .disabled(self.busy() || !self.available || self.meta.is_none())
                    .dropdown_menu(move |mut menu, _, _| {
                        for column in std::iter::once(None).chain(columns.iter().cloned().map(Some))
                        {
                            let owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(
                                    column.clone().unwrap_or_else(|| "清除选择".into()),
                                )
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.update_draft(
                                            epoch,
                                            publication,
                                            |document| {
                                                if x_axis {
                                                    document.encodings.x = column.clone();
                                                } else {
                                                    document.encodings.y = column.clone();
                                                    if histogram {
                                                        document.encodings.x = None;
                                                    }
                                                }
                                            },
                                            window,
                                            cx,
                                        )
                                    });
                                }),
                            );
                        }
                        menu
                    }),
            )
    }
}

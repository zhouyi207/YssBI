mod columns;
pub(super) use columns::PAGE_COLUMNS;

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
        ChartType::Histogram => crate::text::t("chartsSidebar.chartTypes.histogram"),
        ChartType::Scatter => crate::text::t("chartsSidebar.chartTypes.scatter"),
        ChartType::Line => crate::text::t("chartsSidebar.chartTypes.line"),
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
        let datasets = self.catalog.clone();
        let current_name = if self.draft.database_id.is_empty() {
            crate::text::t("native.charts.chooseDataset").into()
        } else {
            datasets
                .databases
                .iter()
                .find(|entry| entry.id == self.draft.database_id)
                .map(|entry| {
                    entry
                        .name
                        .clone()
                        .unwrap_or_else(|| crate::text::translate("native.charts.unnamedDataset"))
                })
                .unwrap_or_else(|| crate::text::t("native.charts.removedDataset").into())
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
                    .child(crate::text::t("native.charts.configuration")),
            )
            .child(
                div()
                    .text_xs()
                    .child(crate::text::t("detail.itemTypes.data")),
            )
            .child(
                Button::new("chart-dataset")
                    .small()
                    .ghost()
                    .label(current_name)
                    .disabled(self.busy() || !self.available)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (id, name) in std::iter::once((
                            String::new(),
                            crate::text::t("native.charts.noDataset").into(),
                        ))
                        .chain(datasets.databases.iter().map(|entry| {
                            (
                                entry.id.clone(),
                                entry.name.clone().unwrap_or_else(|| {
                                    crate::text::translate("native.charts.unnamedDataset")
                                }),
                            )
                        })) {
                            let owner = owner.clone();
                            menu = menu.item(PopupMenuItem::new(name).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.update_draft(
                                            epoch,
                                            publication,
                                            |document| {
                                                if document.database_id != id {
                                                    document.database_id = id.clone();
                                                    document.encodings =
                                                        ChartEncodings { x: None, y: None };
                                                }
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
            .child(
                div()
                    .text_xs()
                    .child(crate::text::t("chartsSidebar.chartType")),
            )
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
                                                if document.chart_type != kind {
                                                    document.chart_type = kind;
                                                    document.encodings =
                                                        ChartEncodings { x: None, y: None };
                                                }
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
        view = view.child(self.render_columns(cx));
        view = view.child(
            div()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if self.external_change {
                    crate::text::t("native.charts.externalChange")
                } else if self.dirty() {
                    crate::text::t("native.charts.unsavedConfiguration")
                } else {
                    crate::text::t("native.workbench.saved")
                }),
        );
        view.into_any_element()
    }
    fn axis_field(&self, x_axis: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let epoch = self.draft_epoch;
        let publication = self.catalog.publication_revision;
        let owner = cx.entity().downgrade();
        let histogram = self.draft.chart_type == ChartType::Histogram;
        let metadata = self.meta.clone();
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
                if metadata.as_ref().is_some_and(|meta| {
                    meta.columns.iter().any(|column| {
                        column.name().as_str() == name && (histogram || numeric(column))
                    })
                }) {
                    name.to_owned()
                } else {
                    crate::text::format(
                        "native.charts.unavailableName",
                        &[("name", name.to_string())],
                    )
                }
            })
            .unwrap_or_else(|| crate::text::t("panel.assistantToolFacts.columns").into());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xs().child(if histogram {
                crate::text::t("native.charts.distributionColumn")
            } else if x_axis {
                crate::text::t("chartsSidebar.encodingX")
            } else {
                crate::text::t("native.charts.yAxis")
            }))
            .child(
                Button::new(if x_axis { "chart-x" } else { "chart-y" })
                    .small()
                    .ghost()
                    .label(label)
                    .disabled(self.busy() || !self.available || self.meta.is_none())
                    .dropdown_menu(move |mut menu, _, _| {
                        let columns = metadata
                            .iter()
                            .flat_map(|meta| meta.columns.iter())
                            .filter(|column| histogram || numeric(column))
                            .map(|column| Some(column.name().as_str().to_owned()));
                        for column in std::iter::once(None).chain(columns) {
                            let owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(column.clone().unwrap_or_else(|| {
                                    crate::text::t("native.charts.clearSelection").into()
                                }))
                                .on_click(
                                    move |_, window, cx| {
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
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
    }
}

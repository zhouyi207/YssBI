mod columns;
pub(super) use columns::PAGE_COLUMNS;

use super::ChartEditor;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, WeakEntity, div, prelude::*};
use std::borrow::Cow;
use yss_chart_document::{ChartDocument, ChartEncodings, ChartType};
use yss_data_contract::{SemanticType, ValueType};
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::ResourceRevision;
use yss_ui::inspector::{
    self, InlineMessage, InspectorHeader, InspectorSection, PropertyField, Severity,
};
use yss_ui::text::translate;

/// Borrowed configuration identity; chart content remains in its retained owner.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    path: Cow<'a, str>,
    epoch: u64,
    source: Option<(Cow<'a, str>, ResourceRevision)>,
    metadata_revision: Option<u64>,
    columns_page: usize,
    busy: bool,
    available: bool,
    dirty: bool,
    external_change: bool,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            path: Cow::Owned(self.path.into_owned()),
            epoch: self.epoch,
            source: self
                .source
                .map(|(id, revision)| (Cow::Owned(id.into_owned()), revision)),
            metadata_revision: self.metadata_revision,
            columns_page: self.columns_page,
            busy: self.busy,
            available: self.available,
            dirty: self.dirty,
            external_change: self.external_change,
        }
    }
}

pub(super) fn chart_type_label(kind: ChartType) -> std::borrow::Cow<'static, str> {
    match kind {
        ChartType::Histogram => yss_ui::text::t("chartsSidebar.chartTypes.histogram"),
        ChartType::Scatter => yss_ui::text::t("chartsSidebar.chartTypes.scatter"),
        ChartType::Line => yss_ui::text::t("chartsSidebar.chartTypes.line"),
    }
}
fn numeric(column: &DatabaseColumnFact) -> bool {
    matches!(
        column.data_type(),
        ValueType::Scalar(SemanticType::Numeric | SemanticType::Datetime)
    )
}
impl ChartEditor {
    /// Configuration epochs identify retained draft/schema data without cloning it.
    pub fn details_state(&self) -> DetailsDisplay<'_> {
        DetailsDisplay {
            path: Cow::Borrowed(self.path.as_str()),
            epoch: self.draft_epoch,
            source: self
                .meta_source
                .as_ref()
                .map(|(id, revision)| (Cow::Borrowed(id.as_str()), *revision)),
            metadata_revision: self.meta.as_ref().map(|meta| meta.data_revision),
            columns_page: self.columns_page,
            busy: self.busy(),
            available: self.available,
            dirty: self.dirty(),
            external_change: self.external_change,
        }
    }

    pub fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                InspectorHeader::new(
                    IconName::ChartLine,
                    self.path.display_name().as_str().to_owned(),
                )
                .detail(translate("detail.itemTypes.chart")),
            )
            .child(self.render_configuration(cx))
            .child(self.render_columns(cx))
            .into_any_element()
    }

    fn render_configuration(&self, cx: &mut Context<Self>) -> InspectorSection {
        let editable = !self.busy() && self.available;
        let epoch = self.draft_epoch;
        let owner = cx.entity().downgrade();
        let datasets = self.catalog.clone();
        let current_name = if self.draft.database_id.is_empty() {
            translate("native.charts.chooseDataset")
        } else {
            datasets
                .databases
                .iter()
                .find(|entry| entry.id == self.draft.database_id)
                .map(|entry| {
                    entry
                        .name
                        .clone()
                        .unwrap_or_else(|| translate("native.charts.unnamedDataset"))
                })
                .unwrap_or_else(|| translate("native.charts.removedDataset"))
        };
        let status = if self.external_change {
            InlineMessage::warning(translate("native.charts.externalChange")).into_any_element()
        } else if self.dirty() {
            InlineMessage::new(
                Severity::Information,
                translate("native.charts.unsavedConfiguration"),
            )
            .into_any_element()
        } else {
            inspector::hint(translate("native.workbench.saved"), cx).into_any_element()
        };
        let kind_owner = owner.clone();
        InspectorSection::new(
            "chart-configuration",
            translate("chartsSidebar.configuration"),
        )
        .child(
            PropertyField::new(translate("detail.itemTypes.data")).control(
                inspector::choice_trigger("chart-dataset", current_name)
                    .disabled(!editable)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (id, name) in
                            std::iter::once((String::new(), translate("native.charts.noDataset")))
                                .chain(datasets.databases.iter().map(|entry| {
                                    (
                                        entry.id.clone(),
                                        entry.name.clone().unwrap_or_else(|| {
                                            translate("native.charts.unnamedDataset")
                                        }),
                                    )
                                }))
                        {
                            menu = menu.item(draft_item(
                                owner.clone(),
                                epoch,
                                name,
                                move |document| {
                                    if document.database_id != id {
                                        document.database_id = id.clone();
                                        document.encodings = ChartEncodings { x: None, y: None };
                                    }
                                },
                            ));
                        }
                        menu
                    }),
            ),
        )
        .child(
            PropertyField::new(translate("chartsSidebar.chartType")).control(
                inspector::choice_trigger("chart-kind", chart_type_label(self.draft.chart_type))
                    .disabled(!editable)
                    .dropdown_menu(move |mut menu, _, _| {
                        for kind in [ChartType::Histogram, ChartType::Scatter, ChartType::Line] {
                            menu = menu.item(draft_item(
                                kind_owner.clone(),
                                epoch,
                                chart_type_label(kind),
                                move |document| {
                                    if document.chart_type != kind {
                                        document.chart_type = kind;
                                        document.encodings = ChartEncodings { x: None, y: None };
                                    }
                                },
                            ));
                        }
                        menu
                    }),
            ),
        )
        .when(self.draft.chart_type != ChartType::Histogram, |section| {
            section.child(self.axis_field(true, cx))
        })
        .child(self.axis_field(false, cx))
        .child(
            div()
                .id("chart-status")
                .role(gpui_kit::accesskit::Role::Status)
                .child(status),
        )
    }

    fn axis_field(&self, x_axis: bool, cx: &mut Context<Self>) -> PropertyField {
        let epoch = self.draft_epoch;
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
                    yss_ui::text::format(
                        "native.charts.unavailableName",
                        &[("name", name.to_string())],
                    )
                }
            })
            .unwrap_or_else(|| translate("panel.assistantToolFacts.columns"));
        PropertyField::new(translate(if histogram {
            "native.charts.distributionColumn"
        } else if x_axis {
            "chartsSidebar.encodingX"
        } else {
            "native.charts.yAxis"
        }))
        .control(
            inspector::choice_trigger(if x_axis { "chart-x" } else { "chart-y" }, label)
                .disabled(self.busy() || !self.available || self.meta.is_none())
                .dropdown_menu(move |mut menu, _, _| {
                    let columns = metadata
                        .iter()
                        .flat_map(|meta| meta.columns.iter())
                        .filter(|column| histogram || numeric(column))
                        .map(|column| Some(column.name().as_str().to_owned()));
                    for column in std::iter::once(None).chain(columns) {
                        let label = column
                            .clone()
                            .unwrap_or_else(|| translate("native.charts.clearSelection"));
                        menu =
                            menu.item(draft_item(owner.clone(), epoch, label, move |document| {
                                if x_axis {
                                    document.encodings.x = column.clone();
                                } else {
                                    document.encodings.y = column.clone();
                                    if histogram {
                                        document.encodings.x = None;
                                    }
                                }
                            }));
                    }
                    menu
                }),
        )
    }
}

/// A menu choice edits the draft captured at render; a newer draft ignores the stale choice.
fn draft_item(
    owner: WeakEntity<ChartEditor>,
    epoch: u64,
    label: impl Into<SharedString>,
    edit: impl Fn(&mut ChartDocument) + 'static,
) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(inspector::owner_command(
        owner,
        move |view: &mut ChartEditor, window, cx| view.update_draft(epoch, &edit, window, cx),
    ))
}

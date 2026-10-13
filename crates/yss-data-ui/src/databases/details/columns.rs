//! A bounded, lazy column directory; callbacks resolve their captured name and revision on use.
use super::{DatabaseEditor, DetailsState};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable,
    collapsible::Collapsible,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement, Window, div, prelude::*};
use yss_application::database::{DatabaseMetaResult, DatabaseMutation};
use yss_data_contract::SemanticType;
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::ResourceRevision;
use yss_ui::inspector::{self, InspectorSection, Pager};
use yss_ui::lifecycle::LogReleased;
use yss_ui::text::translate as t;

const PAGE_COLUMNS: usize = 50;
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

impl DetailsState {
    pub(in crate::databases) fn retain_columns(&mut self, meta: &DatabaseMetaResult) {
        if !self.expanded.is_empty() {
            let names: std::collections::HashSet<_> = meta
                .columns
                .iter()
                .map(|column| column.name().as_str())
                .collect();
            self.expanded.retain(|name| names.contains(name.as_str()));
        }
        self.columns_page = self
            .columns_page
            .min(meta.columns.len().saturating_sub(1) / PAGE_COLUMNS);
    }
}

impl DatabaseEditor {
    pub(super) fn render_columns(
        &self,
        meta: &DatabaseMetaResult,
        cx: &mut Context<Self>,
    ) -> InspectorSection {
        let page = self.details.columns_page;
        let pages = meta.columns.len().div_ceil(PAGE_COLUMNS).max(1);
        let mut list = div().min_w_0().flex().flex_col();
        for (index, column) in meta
            .columns
            .iter()
            .enumerate()
            .skip(page * PAGE_COLUMNS)
            .take(PAGE_COLUMNS)
        {
            let name = column.name().as_str().to_owned();
            let open = self.details.expanded.contains(&name);
            list = list.child(
                Collapsible::new()
                    .w_full()
                    .min_w_0()
                    .open(open)
                    .child(
                        inspector::disclosure(
                            ("database-column-details", index),
                            name.clone(),
                            Some(column.display_type().to_owned().into()),
                            open,
                            cx,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if !view.details.expanded.remove(&name) {
                                view.details.expanded.insert(name.clone());
                            }
                            view.changed(cx);
                        })),
                    )
                    .when(open, |section| {
                        section.content(self.column_settings(column, index, cx))
                    }),
            );
        }
        InspectorSection::new("database-columns", t("detail.fields.columns"))
            .count(meta.columns.len())
            .collapsible(
                self.details.columns_open,
                cx.listener(|view, open: &bool, _, cx| {
                    view.details.columns_open = *open;
                    view.changed(cx);
                }),
            )
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(list)
                    .when(pages > 1, |body| {
                        body.child(div().flex().justify_end().child(
                            Pager::new("columns-pager", page, pages).on_change(cx.listener(
                                |view, page: &usize, _, cx| {
                                    view.details.columns_page = *page;
                                    view.changed(cx);
                                },
                            )),
                        ))
                    }),
            )
    }

    fn column_settings(
        &self,
        column: &DatabaseColumnFact,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let semantic_owner = owner.clone();
        let revision = self.revision;
        let name = column.name().as_str().to_owned();
        let physical_name = name.clone();
        let edit_name = name.clone();
        let current = column.physical_type().to_owned();
        let supported = column.supported_semantic_types().to_vec();
        let kind = column.semantic().map(|semantic| semantic.kind);
        let editable = !self.busy() && self.ready;
        let physical = inspector::choice_trigger(("column-physical", index), current.clone())
            .disabled(!editable)
            .dropdown_menu(move |mut menu, _, _| {
                for dtype in std::iter::once(current.as_str())
                    .chain(PHYSICAL_TYPES.into_iter().filter(|dtype| *dtype != current))
                {
                    let name = physical_name.clone();
                    let before = current.clone();
                    let dtype = dtype.to_owned();
                    menu = menu.item(PopupMenuItem::new(dtype.clone()).on_click(
                        inspector::owner_command(
                            owner.clone(),
                            move |view: &mut DatabaseEditor, window, cx| {
                                view.confirm_physical(
                                    name.clone(),
                                    before.clone(),
                                    dtype.clone(),
                                    revision,
                                    window,
                                    cx,
                                )
                            },
                        ),
                    ));
                }
                menu
            });
        let semantic = inspector::choice_trigger(
            ("column-semantic", index),
            kind.map(|kind| kind.as_str().to_owned())
                .unwrap_or_else(|| t("native.databases.unset")),
        )
        .disabled(!editable || supported.is_empty())
        .dropdown_menu(move |mut menu, _, _| {
            for kind in &supported {
                let kind = *kind;
                let name = name.clone();
                menu = menu.item(PopupMenuItem::new(kind.as_str()).on_click(
                    inspector::owner_command(
                        semantic_owner.clone(),
                        move |view: &mut DatabaseEditor, window, cx| {
                            view.open_column_semantic(&name, kind, revision, window, cx)
                        },
                    ),
                ));
            }
            menu
        });
        let configurable = kind.filter(|kind| {
            column.supported_semantic_types().contains(kind)
                && matches!(
                    kind,
                    SemanticType::Numeric
                        | SemanticType::Categorical
                        | SemanticType::Ordinal
                        | SemanticType::Binary
                )
        });
        div()
            .min_w_0()
            .pl(inspector::DISCLOSURE_INDENT)
            .pt_0p5()
            .pb_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(inspector::sub_field(
                t("native.databases.storageType"),
                physical,
                cx,
            ))
            .child(inspector::sub_field(
                t("native.databases.semantics"),
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().min_w_0().child(semantic))
                    .when_some(configurable, |row, kind| {
                        row.child(
                            inspector::action_button(
                                ("column-semantic-edit", index),
                                IconName::Pencil,
                                t(if kind == SemanticType::Numeric {
                                    "detail.data.editConstraints"
                                } else {
                                    "detail.data.editMapping"
                                }),
                            )
                            .disabled(!editable)
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    view.open_column_semantic(
                                        &edit_name, kind, revision, window, cx,
                                    )
                                },
                            )),
                        )
                    }),
                cx,
            ))
    }

    fn open_column_semantic(
        &self,
        name: &str,
        kind: SemanticType,
        revision: ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.revision != revision || self.busy() || !self.ready {
            return;
        }
        if let Some(column) = self.meta.as_ref().and_then(|meta| {
            meta.columns
                .iter()
                .find(|column| column.name().as_str() == name)
        }) && column.supported_semantic_types().contains(&kind)
        {
            self.semantic_dialog(column, kind, window, cx);
        }
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
        let message = yss_ui::text::format(
            "detail.data.confirmPhysicalMessage",
            &[
                ("column", column.clone()),
                ("from", before.clone()),
                ("to", dtype.clone()),
            ],
        );
        let prompt = yss_ui::modal_window::prompt(
            &t("detail.data.confirmPhysicalTitle"),
            Some(&message),
            &[&t("common.confirm"), &t("common.cancel")],
            window,
            cx,
        );
        self.busy = true;
        let generation = self.generation;
        self.changed(cx);
        cx.spawn_in(window, async move |view, cx| {
            let confirmed = matches!(prompt.await, Ok(0));
            view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                if confirmed {
                    view.mutate(
                        DatabaseMutation::CastColumn {
                            column,
                            dtype,
                            force: false,
                        },
                        revision,
                        window,
                        cx,
                    );
                }
                if view.refresh_again && !view.busy() {
                    view.reload(false, window, cx);
                }
                view.changed(cx);
            })
            .log_released();
        })
        .detach();
    }
}

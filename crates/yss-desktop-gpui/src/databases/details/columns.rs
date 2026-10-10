//! A bounded, lazy column directory; callbacks resolve their captured name and revision on use.
use super::{DatabaseEditor, DetailsState};
use crate::text::translate as t;
use gpui::{Context, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;
use yss_application::database::{DatabaseMetaResult, DatabaseMutation};
use yss_data_contract::SemanticType;
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::ResourceRevision;

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
    ) -> impl IntoElement {
        let page = self.details.columns_page;
        let pages = meta.columns.len().div_ceil(PAGE_COLUMNS).max(1);
        let mut content = div().flex().flex_col().gap_2();
        if self.details.columns_open {
            for (index, column) in meta
                .columns
                .iter()
                .enumerate()
                .skip(page * PAGE_COLUMNS)
                .take(PAGE_COLUMNS)
            {
                let name = column.name().as_str().to_owned();
                let open = self.details.expanded.contains(&name);
                content = content.child(
                    Collapsible::new()
                        .open(open)
                        .child(
                            Button::new(("database-column-details", index))
                                .small()
                                .ghost()
                                .w_full()
                                .label(name.clone())
                                .tooltip(name.clone())
                                .icon(if open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
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
            if pages > 1 {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            Button::new("columns-previous")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(t("databaseEditor.previousPage"))
                                .disabled(page == 0)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.details.columns_page =
                                        view.details.columns_page.saturating_sub(1);
                                    view.changed(cx);
                                })),
                        )
                        .child(div().text_xs().child(format!("{} / {pages}", page + 1)))
                        .child(
                            Button::new("columns-next")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(t("databaseEditor.nextPage"))
                                .disabled(page + 1 >= pages)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.details.columns_page =
                                        (view.details.columns_page + 1).min(pages - 1);
                                    view.changed(cx);
                                })),
                        ),
                );
            }
        }
        Collapsible::new()
            .open(self.details.columns_open)
            .child(
                Button::new("database-columns")
                    .small()
                    .ghost()
                    .w_full()
                    .label(t("detail.fields.columns"))
                    .icon(if self.details.columns_open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.details.columns_open = !view.details.columns_open;
                        view.changed(cx);
                    })),
            )
            .content(content)
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
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pl_3()
            .pb_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t("native.databases.storageType")),
            )
            .child(
                Button::new(("column-physical", index))
                    .small()
                    .outline()
                    .label(current.clone())
                    .disabled(self.busy() || !self.ready)
                    .dropdown_menu(move |mut menu, _, _| {
                        for dtype in std::iter::once(current.as_str())
                            .chain(PHYSICAL_TYPES.into_iter().filter(|dtype| *dtype != current))
                        {
                            let owner = owner.clone();
                            let name = physical_name.clone();
                            let before = current.clone();
                            let dtype = dtype.to_owned();
                            menu = menu.item(PopupMenuItem::new(dtype.clone()).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.confirm_physical(
                                            name.clone(),
                                            before.clone(),
                                            dtype.clone(),
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
                    .child(t("native.databases.semantics")),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new(("column-semantic", index))
                            .small()
                            .outline()
                            .flex_1()
                            .label(
                                kind.map(|kind| kind.as_str().to_owned())
                                    .unwrap_or_else(|| t("native.databases.unset")),
                            )
                            .disabled(self.busy() || !self.ready || supported.is_empty())
                            .dropdown_menu(move |mut menu, _, _| {
                                for kind in &supported {
                                    let kind = *kind;
                                    let owner = semantic_owner.clone();
                                    let name = name.clone();
                                    menu = menu.item(PopupMenuItem::new(kind.as_str()).on_click(
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |view, cx| {
                                                view.open_column_semantic(
                                                    &name, kind, revision, window, cx,
                                                )
                                            });
                                        },
                                    ));
                                }
                                menu
                            }),
                    )
                    .when_some(
                        kind.filter(|kind| {
                            column.supported_semantic_types().contains(kind)
                                && matches!(
                                    kind,
                                    SemanticType::Numeric
                                        | SemanticType::Categorical
                                        | SemanticType::Ordinal
                                        | SemanticType::Binary
                                )
                        }),
                        |view, kind| {
                            view.child(
                                Button::new(("column-semantic-edit", index))
                                    .small()
                                    .ghost()
                                    .icon(IconName::Pencil)
                                    .tooltip(t(if kind == SemanticType::Numeric {
                                        "detail.data.editConstraints"
                                    } else {
                                        "detail.data.editMapping"
                                    }))
                                    .disabled(self.busy() || !self.ready)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.open_column_semantic(
                                            &edit_name, kind, revision, window, cx,
                                        )
                                    })),
                            )
                        },
                    ),
            )
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
        let message = crate::text::format(
            "detail.data.confirmPhysicalMessage",
            &[
                ("column", column.clone()),
                ("from", before.clone()),
                ("to", dtype.clone()),
            ],
        );
        let prompt = crate::modal_window::prompt(
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
            let _ = view.update_in(cx, |view, window, cx| {
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
            });
        })
        .detach();
    }
}

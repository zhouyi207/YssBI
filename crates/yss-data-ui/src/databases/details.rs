mod columns;
use super::DatabaseEditor;
use std::borrow::Cow;
use std::collections::HashSet;
use yss_project_identity::ResourceRevision;

/// The inspector's display dependencies, borrowed for allocation-free comparison.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    name: Cow<'a, str>,
    revision: ResourceRevision,
    metadata: Option<(u64, usize, usize)>,
    busy: bool,
    dirty: bool,
    error: Option<Cow<'a, str>>,
    disclosure: Cow<'a, DetailsState>,
    preview_open: bool,
    summary: Option<(usize, usize, usize)>,
    primary: Option<(usize, usize)>,
    offset: usize,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            name: Cow::Owned(self.name.into_owned()),
            revision: self.revision,
            metadata: self.metadata,
            busy: self.busy,
            dirty: self.dirty,
            error: self.error.map(|error| Cow::Owned(error.into_owned())),
            disclosure: Cow::Owned(self.disclosure.into_owned()),
            preview_open: self.preview_open,
            summary: self.summary,
            primary: self.primary,
            offset: self.offset,
        }
    }
}

use gpui_kit::assets::IconName;
use gpui_kit::component::{Sizable, input::Textarea};
use gpui_kit::{Context, IntoElement, Window, div, prelude::*};
use yss_application::database::DatabaseMetaResult;
use yss_ui::inspector::{self, InlineMessage, InspectorHeader, InspectorSection, Severity};
use yss_ui::text::translate;

#[derive(Clone, PartialEq)]
pub(super) struct DetailsState {
    info_open: bool,
    columns_open: bool,
    columns_page: usize,
    expanded: HashSet<String>,
}
impl Default for DetailsState {
    fn default() -> Self {
        Self {
            info_open: true,
            columns_open: true,
            columns_page: 0,
            expanded: HashSet::new(),
        }
    }
}

impl DatabaseEditor {
    /// Schema revisions and selection describe the inspector without copying table data.
    pub fn details_state(&self, cx: &gpui_kit::App) -> DetailsDisplay<'_> {
        let table = self.grid.read(cx);
        let page = table.delegate();
        let summary = page
            .selection
            .summary(page.rows.row_count(), page.rows.columns().len())
            .filter(|summary| summary.cells > 1)
            .map(|summary| (summary.rows, summary.columns, summary.cells));
        let primary = page
            .selection
            .primary_cell(page.data_selection(table.selection()))
            .filter(|(row, column)| page.value(*row, *column).is_some());
        DetailsDisplay {
            name: Cow::Borrowed(&self.name),
            revision: self.revision,
            metadata: self
                .meta
                .as_ref()
                .map(|meta| (meta.data_revision, meta.row_count, meta.column_count)),
            busy: self.busy(),
            dirty: self.dirty(),
            error: self.error.as_deref().map(Cow::Borrowed),
            disclosure: Cow::Borrowed(&self.details),
            preview_open: self.selection_preview_open,
            summary,
            primary,
            offset: page.offset,
        }
    }

    pub fn render_details(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let _busy = self.busy();
        let view = div().min_w_0().flex().flex_col().child(
            InspectorHeader::new(IconName::Database, self.name.clone())
                .detail(translate("detail.itemTypes.data")),
        );
        let Some(meta) = self.meta.clone() else {
            return view
                .when(self.error.is_none(), |view| {
                    view.child(inspector::status_band(
                        inspector::hint(translate("native.databases.loadingMetadata"), cx),
                        cx,
                    ))
                })
                .into_any_element();
        };
        view.when(self.dirty(), |view| {
            view.child(inspector::status_band(
                InlineMessage::new(
                    Severity::Information,
                    translate("native.databases.checkpointHint"),
                ),
                cx,
            ))
        })
        .child(self.render_info(&meta, cx))
        .child(self.render_columns(&meta, cx))
        .child(self.render_selection(window, cx))
        .into_any_element()
    }

    fn render_info(&self, meta: &DatabaseMetaResult, cx: &mut Context<Self>) -> InspectorSection {
        InspectorSection::new("database-info", translate("detail.sections.info"))
            .collapsible(
                self.details.info_open,
                cx.listener(|view, open: &bool, _, cx| {
                    view.details.info_open = *open;
                    view.changed(cx);
                }),
            )
            .child(inspector::property_list([
                inspector::property_row(
                    translate("detail.fields.columns"),
                    meta.column_count.to_string(),
                    cx,
                ),
                inspector::property_row(
                    translate("detail.fields.rows"),
                    meta.row_count.to_string(),
                    cx,
                ),
            ]))
    }

    /// The preview follows the grid's primary cell; a multi-cell selection adds its extent.
    fn render_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> InspectorSection {
        let section =
            InspectorSection::new("database-selection", translate("databaseEditor.selected"))
                .collapsible(
                    self.selection_preview_open,
                    cx.listener(|view, open: &bool, window, cx| {
                        view.selection_preview_open = *open;
                        view.update_selection_preview(window, cx);
                        view.changed(cx);
                    }),
                );
        if !self.selection_preview_open {
            return section;
        }
        let table = self.grid.read(cx);
        let page = table.delegate();
        let selection = &page.selection;
        let summary = selection
            .summary(page.rows.row_count(), page.rows.columns().len())
            .filter(|summary| summary.cells > 1);
        let primary = selection
            .primary_cell(page.data_selection(table.selection()))
            .filter(|(row, column)| page.value(*row, *column).is_some());
        let row_number = primary
            .map(|(row, _)| (page.offset + row + 1).to_string())
            .unwrap_or_else(|| "—".into());
        let column_name = primary
            .and_then(|(_, column)| page.rows.columns().get(column))
            .map_or_else(
                || "—".to_owned(),
                |column| column.name().as_str().to_owned(),
            );
        let placeholder = translate(
            match primary.and_then(|(row, column)| page.value(row, column)) {
                None => "databaseEditor.cellPreviewPlaceholder",
                Some(yss_data_contract::TabularScalar::Null) => {
                    "databaseEditor.nullCellPlaceholder"
                }
                _ => "databaseEditor.emptyStringPlaceholder",
            },
        );
        if self
            .selection_preview
            .read(cx)
            .presentation()
            .placeholder()
            .as_ref()
            != placeholder.as_str()
        {
            self.selection_preview.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        section
            .when_some(summary, |section, summary| {
                section.child(inspector::hint(
                    yss_ui::text::format(
                        "native.databases.selectionHint",
                        &[
                            ("rows", yss_ui::text::plural("counts.rows", summary.rows)),
                            (
                                "columns",
                                yss_ui::text::plural("counts.columns", summary.columns),
                            ),
                            ("cells", yss_ui::text::plural("counts.cells", summary.cells)),
                        ],
                    ),
                    cx,
                ))
            })
            .child(inspector::property_list([
                inspector::property_row(translate("databaseEditor.rowNumber"), row_number, cx),
                inspector::property_row(translate("databaseEditor.columnName"), column_name, cx),
            ]))
            .child(inspector::sub_field(
                translate("databaseEditor.content"),
                Textarea::new(&self.selection_preview)
                    .readonly(true)
                    .small()
                    .h_32(),
                cx,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::DatabaseEditor;
    use crate::databases::query::{DatabaseRead, PageRead};
    use gpui_kit::component::table::TableSelection;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, AppContext, Context, Entity, IntoElement, Render, Subscription,
        TestAppContext, Window, WindowOptions, div, prelude::*,
    };
    use std::{sync::Arc, time::Duration};
    use yss_application::database::{DatabaseMetaResult, DatabaseRowsResult};
    use yss_application::session::{ApplicationSessionSlot, ApplicationState, NodeComponents};
    use yss_data_contract::{TabularColumn, TabularScalar, TabularSnapshot, ValueType};
    use yss_database_contract::EditState;
    use yss_database_schema::DatabaseColumnFact;
    use yss_desktop_services::{ApplicationTasks, DesktopEvents};
    use yss_project_identity::{ProjectInstanceId, ResourceRevision};

    struct PreviewWindow {
        editor: Entity<DatabaseEditor>,
        _observation: Subscription,
        _runtime: tokio::runtime::Runtime,
    }

    impl Render for PreviewWindow {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let details = self
                .editor
                .update(cx, |editor, cx| editor.render_details(window, cx));
            div().size_full().child(details)
        }
    }

    fn page_read(page_size: usize, offset: usize, page: bool) -> DatabaseRead {
        let values = vec![
            TabularScalar::String("长单元格🦀\n".repeat(1024).into()),
            TabularScalar::String(format!("latest page {offset}").into()),
            TabularScalar::Null,
            TabularScalar::String("".into()),
            TabularScalar::String("tab\tquote\"line\nend".into()),
        ];
        let columns = vec![
            TabularColumn::new("text".try_into().expect("column name"), values.into()),
            TabularColumn::new(
                "number".try_into().expect("column name"),
                (10..15).map(TabularScalar::Integer).collect(),
            ),
        ];
        let meta = DatabaseMetaResult {
            id: "preview".into(),
            name: "Preview".into(),
            row_count: offset + 5,
            column_count: columns.len(),
            columns: columns
                .iter()
                .map(|column| {
                    DatabaseColumnFact::new(column.name().clone(), ValueType::Object, true)
                })
                .collect(),
            data_revision: 1,
        };
        DatabaseRead {
            meta,
            edit: EditState {
                can_undo: false,
                can_redo: false,
                is_modified: false,
                undo_count: 0,
                redo_count: 0,
            },
            page: page.then(|| PageRead {
                rows: DatabaseRowsResult {
                    rows: TabularSnapshot::try_from_columns(columns.into())
                        .expect("rectangular page"),
                    row_ids: (0..5).collect(),
                    has_more: false,
                },
                elapsed: Duration::ZERO,
            }),
            offset,
            page_size,
        }
    }

    fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<DatabaseEditor>) {
        let (window, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::init(cx);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime");
            let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
                NodeComponents::builtins().expect("builtins"),
            )));
            let tasks = Arc::new(ApplicationTasks::new(
                application,
                runtime.handle().clone(),
                DesktopEvents::default(),
            ));
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let editor = cx.new(|cx| {
                        let mut editor = DatabaseEditor::new(
                            tasks,
                            ProjectInstanceId::new(),
                            "preview".into(),
                            ResourceRevision::INITIAL,
                            "Preview".into(),
                            window,
                            cx,
                        );
                        editor.details.info_open = false;
                        editor.details.columns_open = false;
                        editor.install_read(page_read(editor.page_size, 0, true), window, cx);
                        editor
                    });
                    let observation = cx.observe(&editor, |_, _, cx| cx.notify());
                    PreviewWindow {
                        editor,
                        _observation: observation,
                        _runtime: runtime,
                    }
                })
            })
            .expect("preview window")
        });
        let editor = cx.update(|cx| view.read(cx).editor.clone());
        cx.run_until_parked();
        (window, editor)
    }

    fn select(editor: &Entity<DatabaseEditor>, target: TableSelection, cx: &mut TestAppContext) {
        cx.update(|cx| {
            editor
                .read(cx)
                .grid
                .clone()
                .update(cx, |table, cx| table.set_selection(target, cx));
        });
        cx.run_until_parked();
    }

    fn toggle(window: AnyWindowHandle, cx: &mut TestAppContext) {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.within("database-selection").click("header", cx);
        })
        .expect("toggle preview");
        cx.run_until_parked();
    }

    fn text(editor: &Entity<DatabaseEditor>, cx: &mut TestAppContext) -> String {
        cx.update(|cx| {
            editor
                .read(cx)
                .selection_preview
                .read(cx)
                .value()
                .to_string()
        })
    }

    #[gpui_kit::test]
    fn disclosure_reads_latest_hidden_selection_and_preserves_unchanged_text_selection(
        cx: &mut TestAppContext,
    ) {
        let (window, editor) = open(cx);
        select(&editor, TableSelection::Cell(0, 1), cx);
        select(&editor, TableSelection::Cell(1, 1), cx);
        assert_eq!(text(&editor, cx), "");
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "latest page 0");

        cx.update_window(window, |_, window, cx| {
            let preview = editor.read(cx).selection_preview.clone();
            preview.update(cx, |input, cx| input.select_all(window, cx));
            editor.update(cx, |editor, cx| editor.update_selection_preview(window, cx));
            assert_eq!(preview.read(cx).selected_range(), 0.."latest page 0".len());
        })
        .expect("unchanged preview preserves selection");

        toggle(window, cx);
        select(&editor, TableSelection::Cell(0, 1), cx);
        assert_eq!(text(&editor, cx), "latest page 0");
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "长单元格🦀\n".repeat(1024));
    }

    #[gpui_kit::test]
    fn preview_distinguishes_null_empty_and_no_selection_and_formats_scalars(
        cx: &mut TestAppContext,
    ) {
        let (window, editor) = open(cx);
        toggle(window, cx);
        for (target, expected, placeholder) in [
            (
                TableSelection::Cell(1, 1),
                "latest page 0",
                "databaseEditor.emptyStringPlaceholder",
            ),
            (
                TableSelection::Cell(2, 1),
                "",
                "databaseEditor.nullCellPlaceholder",
            ),
            (
                TableSelection::Cell(3, 1),
                "",
                "databaseEditor.emptyStringPlaceholder",
            ),
            (
                TableSelection::Cell(0, 2),
                "10",
                "databaseEditor.emptyStringPlaceholder",
            ),
            (
                TableSelection::None,
                "",
                "databaseEditor.cellPreviewPlaceholder",
            ),
        ] {
            select(&editor, target, cx);
            cx.update_window(window, |_, window, cx| {
                window.render_frame(cx);
                let preview = editor.read(cx).selection_preview.read(cx);
                assert_eq!(preview.text(), expected);
                assert_eq!(
                    preview.presentation().placeholder().as_ref(),
                    yss_ui::text::translate(placeholder).as_str(),
                );
            })
            .expect("preview presentation");
        }
    }

    #[gpui_kit::test]
    fn hidden_page_retention_and_replacement_keep_selection_and_preview_coherent(
        cx: &mut TestAppContext,
    ) {
        let (window, editor) = open(cx);
        select(&editor, TableSelection::Cell(1, 1), cx);
        cx.update_window(window, |_, window, cx| {
            editor.update(cx, |editor, cx| {
                editor.install_read(page_read(editor.page_size, 0, false), window, cx);
            });
        })
        .expect("same-revision metadata refresh");
        cx.run_until_parked();
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "latest page 0");
        toggle(window, cx);

        cx.update_window(window, |_, window, cx| {
            editor.update(cx, |editor, cx| {
                editor.install_read(page_read(editor.page_size, 5, true), window, cx);
            });
        })
        .expect("new page");
        cx.run_until_parked();
        assert_eq!(text(&editor, cx), "latest page 0");
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "");
        cx.update(|cx| {
            let table = editor.read(cx).grid.read(cx);
            assert_eq!(table.selection(), TableSelection::None);
            assert!(table.delegate().selection.is_empty());
        });
        toggle(window, cx);
        select(&editor, TableSelection::Cell(1, 1), cx);
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "latest page 5");
        cx.update_window(window, |_, window, cx| {
            editor.update(cx, |editor, cx| {
                editor.install_read(page_read(editor.page_size, 10, true), window, cx);
                assert_eq!(editor.selection_preview.read(cx).text(), "");
            });
        })
        .expect("open preview clears with new page");
    }

    #[gpui_kit::test]
    fn preview_primary_cell_and_tsv_copy_keep_row_markers_out_of_data(cx: &mut TestAppContext) {
        let (window, editor) = open(cx);
        select(&editor, TableSelection::Cell(4, 0), cx);
        cx.update(|cx| {
            assert_eq!(
                editor.read(cx).grid.read(cx).selection(),
                TableSelection::Cell(4, 1)
            );
        });
        toggle(window, cx);
        assert_eq!(text(&editor, cx), "tab\tquote\"line\nend");
        select(&editor, TableSelection::Row(4), cx);
        cx.update(|cx| {
            editor.update(cx, |editor, cx| {
                editor.copy_selection(cx);
                assert!(editor.grid.read(cx).delegate().selection.row_selected(4));
            });
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("\"tab\tquote\"\"line\nend\"\t14".into()),
            );
        });

        cx.update(|cx| {
            editor.read(cx).grid.clone().update(cx, |table, cx| {
                table
                    .delegate_mut()
                    .selection
                    .begin_cell((1, 0), false, false);
                table.set_selected_cell(3, 2, cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(text(&editor, cx), "13");
        cx.update(|cx| {
            editor.update(cx, |editor, cx| editor.copy_selection(cx));
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("latest page 0\t11\n\t12\n\t13".into()),
            );
        });
    }
}

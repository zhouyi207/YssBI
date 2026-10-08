//! Component selection events feed one page-local highlight and clipboard projection.
mod ranges;
pub(super) use ranges::PageSelection;

use super::{DatabaseEditor, grid::DatabaseGrid};
use gpui::{ClipboardItem, Context, Focusable, Modifiers, Window};
use gpui_component::table::{TableEvent, TableSelection, TableState};
use yss_data_contract::TabularScalar;

impl DatabaseEditor {
    pub(super) fn selection_changed(
        &mut self,
        event: &TableEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = match event {
            TableEvent::SelectCell(row, column) => TableSelection::Cell(*row, *column),
            TableEvent::SelectRow(row) => TableSelection::Row(*row),
            TableEvent::SelectColumn(column) => TableSelection::Column(*column),
            TableEvent::ClearSelection => TableSelection::None,
            _ => return,
        };
        let columns = self.grid.read(cx).delegate().rows.columns().len();
        // The built-in keyboard cursor can enter a non-selectable column. Skip our row marker.
        if columns > 0
            && matches!(
                target,
                TableSelection::Cell(_, 0) | TableSelection::Column(0)
            )
        {
            self.grid.update(cx, |table, cx| match target {
                TableSelection::Cell(row, _) => table.set_selected_cell(row, 1, cx),
                _ => table.set_selected_col(1, cx),
            });
            return;
        }
        let modifiers = window.modifiers();
        let additive =
            !window.last_input_was_keyboard() && (modifiers.control || modifiers.platform);
        let synchronizing = self.selection_cursor_sync.take() == Some(target);
        if !synchronizing {
            self.grid.update(cx, |table, cx| {
                let grid = table.delegate_mut();
                let data_target = grid.data_selection(target);
                if window.last_input_was_keyboard() {
                    grid.selection.dragging = false;
                }
                grid.selection
                    .accept(data_target, modifiers.shift, additive);
                let retained = grid.selection.retained_cursor(data_target);
                if retained != data_target {
                    let retained = table_selection(retained);
                    // Keep the component cursor on a retained index without toggling it twice.
                    self.selection_cursor_sync = Some(retained);
                    table.set_selection(retained, cx);
                }
                cx.notify();
            });
        }
        self.update_selection_preview(window, cx);
        if self.grid.focus_handle(cx).is_focused(window) {
            cx.emit(super::DatabaseEvent::Activated);
        }
        self.changed(cx);
    }
    pub(super) fn select_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.ready {
            return;
        }
        self.grid.update(cx, |table, cx| {
            let grid = table.delegate_mut();
            if grid.rows.row_count() > 0 && !grid.rows.columns().is_empty() {
                grid.selection.select_all();
                cx.notify();
            }
        });
        self.update_selection_preview(window, cx);
        self.changed(cx);
    }
    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selection_cursor_sync = None;
        self.grid.update(cx, |table, cx| {
            table.delegate_mut().selection = PageSelection::default();
            table.clear_selection(cx);
        });
        self.changed(cx);
    }
    pub(super) fn finish_selection_drag(&self, cx: &mut Context<Self>) {
        self.grid.update(cx, |table, _| {
            table.delegate_mut().selection.dragging = false;
        });
    }
    pub(super) fn update_selection_preview(&self, window: &mut Window, cx: &mut Context<Self>) {
        let table = self.grid.read(cx);
        let grid = table.delegate();
        let value = grid
            .selection
            .primary_cell(grid.data_selection(table.selection()))
            .and_then(|(row, column)| grid.value(row, column));
        let text = value
            .filter(|value| !matches!(value, TabularScalar::Null))
            .map(super::grid::display)
            .unwrap_or_default();
        self.selection_preview.update(cx, |input, cx| {
            if input.value() != text {
                input.set_value(text, window, cx);
            }
        });
    }
    pub(super) fn copy_selection(&self, cx: &mut Context<Self>) {
        if !self.ready || self.busy() {
            return;
        }
        let table = self.grid.read(cx);
        let grid = table.delegate();
        let Some((rows, columns)) = grid
            .selection
            .clipboard_axes(grid.rows.row_count(), grid.rows.columns().len())
        else {
            return;
        };
        let text = rows
            .into_iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|column| {
                        let value = grid.value(row, *column);
                        let text = value
                            .filter(|value| !matches!(value, TabularScalar::Null))
                            .map(super::grid::display)
                            .unwrap_or_default();
                        clipboard_cell(&text)
                    })
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}
fn clipboard_cell(value: &str) -> String {
    if value.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.into()
    }
}
pub(super) fn start(
    table: &mut TableState<DatabaseGrid>,
    target: TableSelection,
    modifiers: Modifiers,
    window: &mut Window,
    cx: &mut Context<TableState<DatabaseGrid>>,
) {
    window.focus(&table.focus_handle(cx), cx);
    if let TableSelection::Cell(row, column) = target {
        table.delegate_mut().selection.begin_cell(
            (row, column),
            modifiers.shift,
            modifiers.control || modifiers.platform,
        );
    }
    table.set_selection(table_selection(target), cx);
    cx.stop_propagation();
}

fn table_selection(target: TableSelection) -> TableSelection {
    match target {
        TableSelection::Cell(row, column) => TableSelection::Cell(row, column + 1),
        TableSelection::Column(column) => TableSelection::Column(column + 1),
        other => other,
    }
}

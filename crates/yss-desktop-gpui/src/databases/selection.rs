//! The component owns the active cursor; this extension owns the range anchor and its paint bounds.
use super::{DatabaseEditor, grid::DatabaseGrid};
use gpui::{ClipboardItem, Context, Window};
use gpui_component::table::{TableEvent, TableSelection, TableState};
use std::ops::Range;

#[derive(Clone)]
pub(super) struct SelectionBounds {
    pub rows: Range<usize>,
    pub columns: Range<usize>,
}
impl SelectionBounds {
    pub fn cell_count(&self) -> usize {
        self.rows.len().saturating_mul(self.columns.len())
    }
    pub fn contains(&self, row: usize, column: usize) -> bool {
        self.rows.contains(&row) && self.columns.contains(&column)
    }
}
#[derive(Default)]
pub(super) struct PageSelection {
    anchor: TableSelection,
    pub bounds: Option<SelectionBounds>,
    pub all: bool,
    pub dragging: bool,
    extend_next: bool,
}
impl PageSelection {
    fn accept(&mut self, target: TableSelection, extend: bool, rows: usize, columns: usize) {
        let pending_extend = std::mem::take(&mut self.extend_next);
        let extend = extend || self.dragging || pending_extend;
        self.all = false;
        if target == TableSelection::None {
            *self = Self::default();
            return;
        }
        if !extend || std::mem::discriminant(&self.anchor) != std::mem::discriminant(&target) {
            self.anchor = target;
        }
        let (row_range, column_range) = match (self.anchor, target) {
            (TableSelection::Cell(a_row, a_col), TableSelection::Cell(row, col)) => {
                (between(a_row, row, rows), between(a_col, col, columns))
            }
            (TableSelection::Row(anchor), TableSelection::Row(row)) => {
                (between(anchor, row, rows), 0..columns)
            }
            (TableSelection::Column(anchor), TableSelection::Column(column)) => {
                (0..rows, between(anchor, column, columns))
            }
            _ => return,
        };
        self.bounds =
            (!row_range.is_empty() && !column_range.is_empty()).then_some(SelectionBounds {
                rows: row_range,
                columns: column_range,
            });
    }
    pub fn begin_cell(&mut self, row: usize, column: usize, extend: bool) {
        if !extend {
            self.anchor = TableSelection::Cell(row, column);
        }
        self.dragging = true;
        self.extend_next = true;
    }
    pub fn extend_cell(&mut self) {
        self.extend_next = true;
    }
    fn select_all(&mut self, rows: usize, columns: usize) {
        *self = Self {
            all: true,
            bounds: (rows > 0 && columns > 0).then_some(SelectionBounds {
                rows: 0..rows,
                columns: 0..columns,
            }),
            ..Self::default()
        };
    }
}
fn between(anchor: usize, target: usize, limit: usize) -> Range<usize> {
    anchor.min(target).min(limit)..anchor.max(target).saturating_add(1).min(limit)
}
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
        let extend = window.modifiers().shift;
        self.grid.update(cx, |table, cx| {
            let grid = table.delegate_mut();
            grid.selection.accept(
                target,
                extend,
                grid.rows.row_count(),
                grid.rows.columns().len(),
            );
            cx.notify();
        });
        cx.emit(super::DatabaseEvent::Activated);
        self.changed(cx);
    }
    pub(super) fn select_page(&mut self, cx: &mut Context<Self>) {
        if self.busy() || !self.ready {
            return;
        }
        self.grid.update(cx, |table, cx| {
            let grid = table.delegate_mut();
            grid.selection
                .select_all(grid.rows.row_count(), grid.rows.columns().len());
            cx.notify();
        });
        self.changed(cx);
    }
    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn copy_selection(&self, cx: &mut Context<Self>) {
        if !self.ready || self.busy() {
            return;
        }
        let table = self.grid.read(cx);
        let grid = table.delegate();
        let Some(bounds) = &grid.selection.bounds else {
            return;
        };
        let text = bounds
            .rows
            .clone()
            .map(|row| {
                bounds
                    .columns
                    .clone()
                    .map(|column| clipboard_cell(&grid.text(row, column)))
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
pub(super) fn start_cell(
    table: &mut TableState<DatabaseGrid>,
    row: usize,
    column: usize,
    extend: bool,
    window: &mut Window,
    cx: &mut Context<TableState<DatabaseGrid>>,
) {
    use gpui::Focusable;
    window.focus(&table.focus_handle(cx), cx);
    table
        .delegate_mut()
        .selection
        .begin_cell(row, column, extend);
    table.set_selected_cell(row, column, cx);
    cx.stop_propagation();
}

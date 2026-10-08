//! Page-local highlights extend the component cursor without storing another active cell.
use gpui_component::table::TableSelection;
use std::{collections::BTreeSet, ops::Range};

#[derive(Clone, PartialEq, Eq)]
struct CellRange {
    rows: Range<usize>,
    columns: Range<usize>,
}
impl CellRange {
    fn between(anchor: (usize, usize), target: (usize, usize)) -> Self {
        Self {
            rows: between(anchor.0, target.0),
            columns: between(anchor.1, target.1),
        }
    }
    fn contains(&self, row: usize, column: usize) -> bool {
        self.rows.contains(&row) && self.columns.contains(&column)
    }
}

#[derive(Default)]
enum Highlight {
    #[default]
    None,
    Cells(Vec<CellRange>),
    Rows(BTreeSet<usize>),
    Columns(BTreeSet<usize>),
    All,
}

pub(in crate::databases) struct SelectionSummary {
    pub rows: usize,
    pub columns: usize,
    pub cells: usize,
}

#[derive(Default)]
pub(in crate::databases) struct PageSelection {
    anchor: TableSelection,
    highlight: Highlight,
    pub dragging: bool,
}
impl PageSelection {
    pub fn is_empty(&self) -> bool {
        match &self.highlight {
            Highlight::None => true,
            Highlight::Rows(indices) | Highlight::Columns(indices) => indices.is_empty(),
            Highlight::Cells(ranges) => ranges.is_empty(),
            Highlight::All => false,
        }
    }
    pub fn contains(&self, row: usize, column: usize) -> bool {
        match &self.highlight {
            Highlight::None => false,
            Highlight::Cells(ranges) => ranges.iter().any(|range| range.contains(row, column)),
            Highlight::Rows(indices) => indices.contains(&row),
            Highlight::Columns(indices) => indices.contains(&column),
            Highlight::All => true,
        }
    }
    pub fn row_selected(&self, row: usize) -> bool {
        matches!(&self.highlight, Highlight::Rows(indices) if indices.contains(&row))
            || matches!(self.highlight, Highlight::All)
    }
    pub fn column_selected(&self, column: usize) -> bool {
        matches!(&self.highlight, Highlight::Columns(indices) if indices.contains(&column))
            || matches!(self.highlight, Highlight::All)
    }
    pub fn primary_cell(&self, cursor: TableSelection) -> Option<(usize, usize)> {
        match &self.highlight {
            Highlight::Cells(ranges) => {
                let range = ranges.last()?;
                if let TableSelection::Cell(row, column) = cursor
                    && range.contains(row, column)
                {
                    Some((row, column))
                } else {
                    Some((range.rows.start, range.columns.start))
                }
            }
            Highlight::Rows(indices) => indices.first().map(|row| (*row, 0)),
            Highlight::Columns(indices) => indices.first().map(|column| (0, *column)),
            Highlight::All => Some((0, 0)),
            Highlight::None => None,
        }
    }
    pub fn summary(&self, rows: usize, columns: usize) -> Option<SelectionSummary> {
        if self.is_empty() {
            return None;
        }
        let (rows, columns, cells) = match &self.highlight {
            Highlight::None => return None,
            Highlight::Rows(indices) => (indices.len(), columns, indices.len() * columns),
            Highlight::Columns(indices) => (rows, indices.len(), rows * indices.len()),
            Highlight::All => (rows, columns, rows * columns),
            Highlight::Cells(ranges) if ranges.len() == 1 => {
                let range = &ranges[0];
                (
                    range.rows.len(),
                    range.columns.len(),
                    range.rows.len() * range.columns.len(),
                )
            }
            Highlight::Cells(ranges) => {
                // Merge intervals on the bounded page; never materialize selected cell values.
                let cells = (0..rows)
                    .map(|row| {
                        union_len(
                            ranges
                                .iter()
                                .filter(|range| range.rows.contains(&row))
                                .map(|range| range.columns.clone()),
                        )
                    })
                    .sum();
                (
                    union_len(ranges.iter().map(|range| range.rows.clone())),
                    union_len(ranges.iter().map(|range| range.columns.clone())),
                    cells,
                )
            }
        };
        Some(SelectionSummary {
            rows,
            columns,
            cells,
        })
    }
    pub fn clipboard_axes(&self, rows: usize, columns: usize) -> Option<(Vec<usize>, Vec<usize>)> {
        if self.is_empty() || rows == 0 || columns == 0 {
            return None;
        }
        Some(match &self.highlight {
            Highlight::None => return None,
            // Like the reference grid, copy only the active rectangle of a cell multi-selection.
            Highlight::Cells(ranges) => {
                let range = ranges.last()?;
                (
                    range.rows.clone().collect(),
                    range.columns.clone().collect(),
                )
            }
            Highlight::Rows(indices) => (indices.iter().copied().collect(), (0..columns).collect()),
            Highlight::Columns(indices) => ((0..rows).collect(), indices.iter().copied().collect()),
            Highlight::All => ((0..rows).collect(), (0..columns).collect()),
        })
    }
    pub fn begin_cell(&mut self, target: (usize, usize), extend: bool, additive: bool) {
        let anchor = match self.anchor {
            TableSelection::Cell(row, column) if extend => (row, column),
            _ => target,
        };
        self.anchor = TableSelection::Cell(anchor.0, anchor.1);
        let mut ranges = match std::mem::take(&mut self.highlight) {
            Highlight::Cells(ranges) if additive => ranges,
            _ => vec![],
        };
        let range = CellRange::between(anchor, target);
        ranges.retain(|existing| *existing != range);
        ranges.push(range);
        self.highlight = Highlight::Cells(ranges);
        self.dragging = true;
    }
    pub fn accept(&mut self, target: TableSelection, extend: bool, additive: bool) {
        match target {
            TableSelection::None => *self = Self::default(),
            TableSelection::Cell(row, column) => {
                let anchor = match self.anchor {
                    TableSelection::Cell(a_row, a_column) if extend || self.dragging => {
                        (a_row, a_column)
                    }
                    _ => (row, column),
                };
                self.anchor = TableSelection::Cell(anchor.0, anchor.1);
                let range = CellRange::between(anchor, (row, column));
                if self.dragging
                    && let Highlight::Cells(ranges) = &mut self.highlight
                    && let Some(current) = ranges.last_mut()
                {
                    *current = range;
                } else {
                    self.highlight = Highlight::Cells(vec![range]);
                }
            }
            TableSelection::Row(row) => {
                let anchor = match self.anchor {
                    TableSelection::Row(anchor) => Some(anchor),
                    _ => None,
                };
                let current = match std::mem::take(&mut self.highlight) {
                    Highlight::Rows(indices) => indices,
                    _ => BTreeSet::new(),
                };
                self.highlight =
                    Highlight::Rows(update_indices(current, row, anchor, extend, additive));
                if !extend || anchor.is_none() {
                    self.anchor = target;
                }
                self.dragging = false;
            }
            TableSelection::Column(column) => {
                let anchor = match self.anchor {
                    TableSelection::Column(anchor) => Some(anchor),
                    _ => None,
                };
                let current = match std::mem::take(&mut self.highlight) {
                    Highlight::Columns(indices) => indices,
                    _ => BTreeSet::new(),
                };
                self.highlight =
                    Highlight::Columns(update_indices(current, column, anchor, extend, additive));
                if !extend || anchor.is_none() {
                    self.anchor = target;
                }
                self.dragging = false;
            }
        }
        if self.is_empty() {
            *self = Self::default();
        }
    }
    pub fn retained_cursor(&self, cursor: TableSelection) -> TableSelection {
        match (&self.highlight, cursor) {
            (Highlight::None, _) => TableSelection::None,
            (Highlight::Rows(indices), TableSelection::Row(row)) if !indices.contains(&row) => {
                indices
                    .first()
                    .copied()
                    .map(TableSelection::Row)
                    .unwrap_or_default()
            }
            (Highlight::Columns(indices), TableSelection::Column(column))
                if !indices.contains(&column) =>
            {
                indices
                    .first()
                    .copied()
                    .map(TableSelection::Column)
                    .unwrap_or_default()
            }
            _ => cursor,
        }
    }
    pub fn select_all(&mut self) {
        *self = Self {
            anchor: TableSelection::Cell(0, 0),
            highlight: Highlight::All,
            dragging: false,
        };
    }
}

fn between(anchor: usize, target: usize) -> Range<usize> {
    anchor.min(target)..anchor.max(target) + 1
}
fn update_indices(
    mut current: BTreeSet<usize>,
    target: usize,
    anchor: Option<usize>,
    extend: bool,
    additive: bool,
) -> BTreeSet<usize> {
    if extend && let Some(anchor) = anchor {
        if !additive {
            current.clear();
        }
        current.extend(between(anchor, target));
    } else if additive {
        if !current.remove(&target) {
            current.insert(target);
        }
    } else {
        current = BTreeSet::from([target]);
    }
    current
}
fn union_len(ranges: impl Iterator<Item = Range<usize>>) -> usize {
    let mut ranges: Vec<_> = ranges.collect();
    ranges.sort_unstable_by_key(|range| range.start);
    let mut end = 0;
    let mut length = 0;
    for range in ranges {
        length += range.end.saturating_sub(end.max(range.start));
        end = end.max(range.end);
    }
    length
}

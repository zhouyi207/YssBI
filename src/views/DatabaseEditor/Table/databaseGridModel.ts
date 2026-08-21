import type { DatabaseGridSelection } from '@/features/application/databaseEditor';
import type { DatabaseRow } from '@/shared/types/dto/database';

export interface DatabaseGridRow {
  values: DatabaseRow;
  rowId: number | string;
  sourceRowIndex: number;
}

export interface DatabaseGridCellAddress {
  row: number;
  column: number;
}

export interface DatabaseGridCellRange extends DatabaseGridCellAddress {
  rowCount: number;
  columnCount: number;
}

export interface DatabaseGridSelectionModifiers {
  additive: boolean;
  extend: boolean;
}

export type DatabaseColumnKind = 'number' | 'boolean' | 'string';

const DATA_COLUMN_PREFIX = 'data_';

export function dataColumnId(columnIndex: number): string {
  return `${DATA_COLUMN_PREFIX}${columnIndex}`;
}

export function dataColumnIndexFromId(columnId: string): number | null {
  if (!columnId.startsWith(DATA_COLUMN_PREFIX)) return null;
  const columnIndex = Number(columnId.slice(DATA_COLUMN_PREFIX.length));
  return Number.isInteger(columnIndex) && columnIndex >= 0 ? columnIndex : null;
}

export function databaseColumnKind(dtype?: string): DatabaseColumnKind {
  const normalized = (dtype ?? '').toLowerCase();
  if (
    normalized.includes('int')
    || normalized.includes('float')
    || normalized.includes('double')
    || normalized.includes('number')
    || normalized.includes('decimal')
  ) {
    return 'number';
  }
  if (normalized.includes('bool')) return 'boolean';
  return 'string';
}

export function createCellRange(
  start: DatabaseGridCellAddress,
  end: DatabaseGridCellAddress,
): DatabaseGridCellRange {
  const row = Math.min(start.row, end.row);
  const column = Math.min(start.column, end.column);
  return {
    row,
    column,
    rowCount: Math.abs(start.row - end.row) + 1,
    columnCount: Math.abs(start.column - end.column) + 1,
  };
}

export function createKeyboardCellSelection(
  selection: DatabaseGridSelection | null,
  anchor: DatabaseGridCellAddress | null,
  target: DatabaseGridCellAddress,
  extend: boolean,
) {
  const nextAnchor = extend
    ? anchor ?? (selection?.type === 'cells' ? selection.activeCell : target)
    : target;
  return {
    anchor: nextAnchor,
    selection: {
      type: 'cells' as const,
      activeCell: target,
      ranges: [createCellRange(nextAnchor, target)],
    },
  };
}

function rangeContainsCell(
  range: DatabaseGridCellRange,
  row: number,
  column: number,
): boolean {
  return row >= range.row
    && row < range.row + range.rowCount
    && column >= range.column
    && column < range.column + range.columnCount;
}

export function isGridCellSelected(
  selection: DatabaseGridSelection | null,
  row: number,
  column: number,
): boolean {
  if (!selection) return false;
  if (selection.type === 'columns') return selection.columns.includes(column);
  if (selection.type !== 'cells') return false;
  return selection.ranges.some((range) => rangeContainsCell(range, row, column));
}

export function isGridCellActive(
  selection: DatabaseGridSelection | null,
  row: number,
  column: number,
): boolean {
  return selection?.type === 'cells'
    && selection.activeCell.row === row
    && selection.activeCell.column === column;
}

export function isGridColumnSelected(
  selection: DatabaseGridSelection | null,
  column: number,
): boolean {
  return selection?.type === 'columns' && selection.columns.includes(column);
}

export function updateIndexSelection(
  current: readonly number[],
  index: number,
  anchor: number | null,
  modifiers: DatabaseGridSelectionModifiers,
  itemCount: number,
): number[] {
  if (!Number.isInteger(index) || index < 0 || index >= itemCount) return [...current];

  if (modifiers.extend && anchor !== null && anchor >= 0 && anchor < itemCount) {
    const start = Math.min(anchor, index);
    const end = Math.max(anchor, index);
    const range = Array.from({ length: end - start + 1 }, (_, offset) => start + offset);
    return modifiers.additive ? [...new Set([...current, ...range])] : range;
  }

  if (!modifiers.additive) return [index];
  return current.includes(index)
    ? current.filter((value) => value !== index)
    : [...current, index];
}

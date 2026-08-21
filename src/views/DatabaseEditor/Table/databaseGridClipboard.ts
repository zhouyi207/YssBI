import type { DatabaseGridSelection } from '@/features/application/databaseEditor';
import type { DatabaseRow } from '@/shared/types/dto/database';
import type { DatabaseGridCellRange } from './databaseGridModel';

function clipboardCell(value: unknown): string {
  if (value === null || value === undefined) return '';
  const text = String(value);
  return /["\t\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function serializeRows(rows: readonly (readonly unknown[])[]): string {
  return rows.map((row) => row.map(clipboardCell).join('\t')).join('\n');
}

function primaryCellRange(selection: Extract<DatabaseGridSelection, { type: 'cells' }>) {
  const active = selection.activeCell;
  return [...selection.ranges].reverse().find((range) => (
    active.row >= range.row
    && active.row < range.row + range.rowCount
    && active.column >= range.column
    && active.column < range.column + range.columnCount
  )) ?? selection.ranges[selection.ranges.length - 1] ?? {
    row: active.row,
    column: active.column,
    rowCount: 1,
    columnCount: 1,
  };
}

function rowsInRange(
  rows: readonly DatabaseRow[],
  range: DatabaseGridCellRange,
  columnCount: number,
): DatabaseRow[] {
  const startRow = Math.max(0, range.row);
  const endRow = Math.min(rows.length, range.row + range.rowCount);
  const startColumn = Math.max(0, range.column);
  const endColumn = Math.min(columnCount, range.column + range.columnCount);
  const selectedRows: DatabaseRow[] = [];

  for (let row = startRow; row < endRow; row += 1) {
    selectedRows.push(rows[row]?.slice(startColumn, endColumn) ?? []);
  }
  return selectedRows;
}

export function databaseGridSelectionToClipboardText(
  selection: DatabaseGridSelection | null,
  rows: readonly DatabaseRow[],
  columnCount: number,
): string | null {
  if (!selection || rows.length === 0 || columnCount <= 0) return null;

  if (selection.type === 'cells') {
    return serializeRows(rowsInRange(rows, primaryCellRange(selection), columnCount));
  }

  if (selection.type === 'rows') {
    const selectedRows = selection.rows
      .filter((rowIndex) => rowIndex >= 0 && rowIndex < rows.length)
      .map((rowIndex) => rows[rowIndex]?.slice(0, columnCount) ?? []);
    return selectedRows.length > 0 ? serializeRows(selectedRows) : null;
  }

  const selectedColumns = selection.columns.filter(
    (columnIndex) => columnIndex >= 0 && columnIndex < columnCount,
  );
  if (selectedColumns.length === 0) return null;
  return serializeRows(rows.map((row) => (
    selectedColumns.map((columnIndex) => row[columnIndex])
  )));
}

function parsePlainTsv(text: string): string[][] {
  const lines = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n');
  if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
  return lines.map((line) => line.split('\t'));
}

export function parseDatabaseGridClipboard(text: string): string[][] {
  if (text.length === 0) return [];

  const rows: string[][] = [[]];
  let value = '';
  let quoted = false;
  let atFieldStart = true;

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (character === '"') {
      if (quoted && text[index + 1] === '"') {
        value += '"';
        index += 1;
      } else if (quoted) {
        const nextCharacter = text[index + 1];
        if (nextCharacter !== undefined
          && nextCharacter !== '\t'
          && nextCharacter !== '\n'
          && nextCharacter !== '\r') {
          return parsePlainTsv(text);
        }
        quoted = false;
      } else if (atFieldStart) {
        quoted = true;
        atFieldStart = false;
      } else {
        value += '"';
      }
      continue;
    }
    if (!quoted && character === '\t') {
      rows[rows.length - 1]?.push(value);
      value = '';
      atFieldStart = true;
      continue;
    }
    if (!quoted && (character === '\n' || character === '\r')) {
      if (character === '\r' && text[index + 1] === '\n') index += 1;
      rows[rows.length - 1]?.push(value);
      rows.push([]);
      value = '';
      atFieldStart = true;
      continue;
    }
    value += character;
    atFieldStart = false;
  }

  if (quoted) return parsePlainTsv(text);

  const lastRow = rows[rows.length - 1];
  lastRow?.push(value);
  if (rows.length > 1 && lastRow?.length === 1 && lastRow[0] === '') {
    rows.pop();
  }
  return rows;
}

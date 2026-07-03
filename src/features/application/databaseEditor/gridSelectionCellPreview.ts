import type { GridSelection } from '@glideapps/glide-data-grid';
import { isEmptyGridSelection } from './useSelection';

function formatCellForPreview(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return '';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

/**
 * 用于标题栏等处的只读预览：优先 `current.cell`；仅有行选时取首行第 0 列；仅有列选时取第 0 行该列。
 */
export function getGridSelectionPrimaryCellText(
  selection: GridSelection | null,
  columnCount: number,
  rowCount: number,
  loadedRows: readonly (readonly unknown[])[],
): string {
  if (!selection || isEmptyGridSelection(selection)) return '';

  const cur = selection.current;
  if (cur) {
    const [col, row] = cur.cell;
    if (row < 0 || col < 0 || col >= columnCount) return '';
    if (row >= rowCount) return '';
    const rowData = loadedRows[row] as readonly unknown[] | undefined;
    if (!rowData) return '';
    return formatCellForPreview(rowData[col]);
  }

  const firstRow = selection.rows.first();
  if (firstRow !== undefined && columnCount > 0) {
    if (firstRow < 0 || firstRow >= rowCount) return '';
    const rowData = loadedRows[firstRow] as readonly unknown[] | undefined;
    if (rowData?.length) return formatCellForPreview(rowData[0]);
  }

  const firstCol = selection.columns.first();
  if (firstCol !== undefined && rowCount > 0) {
    if (firstCol < 0 || firstCol >= columnCount) return '';
    const rowData = loadedRows[0] as readonly unknown[] | undefined;
    if (rowData) return formatCellForPreview(rowData[firstCol]);
  }

  return '';
}

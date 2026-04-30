import { useCallback, useState } from 'react';

export interface CellPos { row: number; col: number; }
export interface SelectionRange { anchor: CellPos; end: CellPos; }

export function selectionBounds(sel: SelectionRange) {
  return {
    r0: Math.min(sel.anchor.row, sel.end.row),
    r1: Math.max(sel.anchor.row, sel.end.row),
    c0: Math.min(sel.anchor.col, sel.end.col),
    c1: Math.max(sel.anchor.col, sel.end.col),
  };
}

interface UseSelectionParams {
  columnCount: number;
  rowCount: number;
}

export function useSelection({ columnCount, rowCount }: UseSelectionParams) {
  const [selection, setSelection] = useState<SelectionRange | null>(null);

  const selectedRowIndices = useCallback((): number[] => {
    if (!selection) return [];
    const { r0, r1, c0, c1 } = selectionBounds(selection);
    if (c0 !== 0 || c1 < columnCount - 1) return [];
    const start = Math.max(0, r0);
    const end = Math.min(rowCount - 1, r1);
    if (end < start) return [];
    const rows: number[] = [];
    for (let r = start; r <= end; r++) rows.push(r);
    return rows;
  }, [selection, columnCount, rowCount]);

  const selectAll = useCallback(() => {
    if (rowCount > 0 && columnCount > 0) {
      setSelection({ anchor: { row: 0, col: 0 }, end: { row: rowCount - 1, col: columnCount - 1 } });
    }
  }, [rowCount, columnCount]);

  const clearSelection = useCallback(() => setSelection(null), []);

  return {
    selection,
    setSelection,
    selectedRowIndices,
    selectAll,
    clearSelection,
  };
}

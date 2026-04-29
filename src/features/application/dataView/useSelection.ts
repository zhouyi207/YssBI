import { useState, useCallback, useEffect } from 'react';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

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
  isEditing: boolean;
}

export function useSelection({ columnCount, rowCount, isEditing }: UseSelectionParams) {
  const [selection, setSelection] = useState<SelectionRange | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const activeCell = selection?.end ?? null;

  const isInSelection = useCallback((row: number, col: number) => {
    if (!selection) return false;
    const { r0, r1, c0, c1 } = selectionBounds(selection);
    return row >= r0 && row <= r1 && col >= c0 && col <= c1;
  }, [selection]);

  const selectedRowIndices = useCallback((): number[] => {
    if (!selection) return [];
    const { r0, r1, c0, c1 } = selectionBounds(selection);
    if (c0 !== 0 || c1 < columnCount - 1) return [];
    const rows: number[] = [];
    for (let r = r0; r <= r1; r++) rows.push(r);
    return rows;
  }, [selection, columnCount]);

  const handleCellMouseDown = useCallback((row: number, col: number, e: React.MouseEvent) => {
    if (e.button !== 0 || isEditing) return;
    if (e.shiftKey && selection) {
      setSelection({ anchor: selection.anchor, end: { row, col } });
    } else {
      setSelection({ anchor: { row, col }, end: { row, col } });
    }
    setIsDragging(true);
  }, [isEditing, selection]);

  const handleCellMouseEnter = useCallback((row: number, col: number) => {
    if (!isDragging || !selection) return;
    setSelection({ anchor: selection.anchor, end: { row, col } });
  }, [isDragging, selection]);

  useEffect(() => {
    if (!isDragging) return;
    const up = () => setIsDragging(false);
    return addGlobalEventListener(window, 'mouseup', up);
  }, [isDragging]);

  const handleRowHeaderClick = useCallback((row: number, e: React.MouseEvent) => {
    const lastCol = columnCount - 1;
    if (lastCol < 0) return;
    if (e.shiftKey && selection) {
      setSelection({ anchor: { ...selection.anchor, col: 0 }, end: { row, col: lastCol } });
    } else {
      setSelection({ anchor: { row, col: 0 }, end: { row, col: lastCol } });
    }
  }, [columnCount, selection]);

  const handleColHeaderClick = useCallback((col: number, e: React.MouseEvent) => {
    const lastRow = rowCount - 1;
    if (lastRow < 0) return;
    if (e.shiftKey && selection) {
      setSelection({ anchor: { ...selection.anchor, row: 0 }, end: { row: lastRow, col } });
    } else {
      setSelection({ anchor: { row: 0, col }, end: { row: lastRow, col } });
    }
  }, [rowCount, selection]);

  const selectAll = useCallback(() => {
    if (rowCount > 0 && columnCount > 0) {
      setSelection({ anchor: { row: 0, col: 0 }, end: { row: rowCount - 1, col: columnCount - 1 } });
    }
  }, [rowCount, columnCount]);

  const clearSelection = useCallback(() => setSelection(null), []);

  return {
    selection,
    setSelection,
    isDragging,
    activeCell,
    isInSelection,
    selectedRowIndices,
    handleCellMouseDown,
    handleCellMouseEnter,
    handleRowHeaderClick,
    handleColHeaderClick,
    selectAll,
    clearSelection,
  };
}

import { useEffect } from 'react';
import type { CellPos, SelectionRange } from './useSelection';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

interface UseDataViewKeyboardParams {
  handleUndo: () => void;
  handleRedo: () => void;
  cancelEdit: () => void;
  startEdit: (row: number, col: number) => void;
  handleDeleteRow: (indices: number[]) => Promise<void>;
  selectAll: () => void;
  clearSelection: () => void;
  setSelection: (sel: SelectionRange | null) => void;
  dismissContextMenu: () => void;
  selection: SelectionRange | null;
  activeCell: CellPos | null;
  editingCell: { row: number; col: number } | null;
  selectedRowIndices: () => number[];
  rowCount: number;
  columnCount: number;
}

export function useDataViewKeyboard({
  handleUndo,
  handleRedo,
  cancelEdit,
  startEdit,
  handleDeleteRow,
  selectAll,
  clearSelection,
  setSelection,
  dismissContextMenu,
  selection,
  activeCell,
  editingCell,
  selectedRowIndices,
  rowCount,
  columnCount,
}: UseDataViewKeyboardParams) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'z' && !e.shiftKey) {
        e.preventDefault(); handleUndo();
      } else if ((e.ctrlKey && e.shiftKey && e.key === 'Z') || (e.ctrlKey && e.key === 'y')) {
        e.preventDefault(); handleRedo();
      } else if (e.key === 'Escape') {
        cancelEdit(); dismissContextMenu(); clearSelection();
      } else if (e.ctrlKey && e.key === 'a' && !editingCell) {
        e.preventDefault(); selectAll();
      } else if (e.key === 'Delete' && !editingCell && selection) {
        const rows = selectedRowIndices();
        if (rows.length > 0) { e.preventDefault(); handleDeleteRow(rows); clearSelection(); }
      } else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key) && !editingCell) {
        e.preventDefault();
        const cur = activeCell ?? { row: 0, col: 0 };
        let nr = cur.row, nc = cur.col;
        if (e.key === 'ArrowUp') nr = Math.max(0, cur.row - 1);
        else if (e.key === 'ArrowDown') nr = Math.min(rowCount - 1, cur.row + 1);
        else if (e.key === 'ArrowLeft') nc = Math.max(0, cur.col - 1);
        else if (e.key === 'ArrowRight') nc = Math.min(columnCount - 1, cur.col + 1);
        if (e.shiftKey && selection) {
          setSelection({ anchor: selection.anchor, end: { row: nr, col: nc } });
        } else {
          setSelection({ anchor: { row: nr, col: nc }, end: { row: nr, col: nc } });
        }
      } else if ((e.key === 'Enter' || e.key === 'F2') && !editingCell && activeCell) {
        e.preventDefault(); startEdit(activeCell.row, activeCell.col);
      }
    };
    return addGlobalEventListener(window, 'keydown', handler);
  }, [
    handleUndo, handleRedo, cancelEdit, startEdit, handleDeleteRow,
    selectAll, clearSelection, setSelection, dismissContextMenu,
    selection, activeCell, editingCell, selectedRowIndices, rowCount, columnCount,
  ]);
}

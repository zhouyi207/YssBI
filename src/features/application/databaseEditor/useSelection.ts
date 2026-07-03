import { useCallback, useState } from 'react';
import { CompactSelection, type GridSelection } from '@glideapps/glide-data-grid';

/** 与 Glide DataEditor 内部一致的空选区（列坐标不含行号列偏移） */
export const emptyGridSelection: GridSelection = {
  columns: CompactSelection.empty(),
  rows: CompactSelection.empty(),
  current: undefined,
};

export function isEmptyGridSelection(s: GridSelection | null | undefined): boolean {
  if (s === null || s === undefined) return true;
  return s.rows.length === 0 && s.columns.length === 0 && s.current === undefined;
}

interface UseSelectionParams {
  columnCount: number;
  rowCount: number;
}

/**
 * 选区状态与 Glide `onGridSelectionChange` 传出结构一致，不做平行领域模型，避免与 Ctrl/Shift 行选语义脱节。
 */
export function useSelection({ columnCount, rowCount }: UseSelectionParams) {
  const [selection, setSelection] = useState<GridSelection | null>(null);

  const selectedRowIndices = useCallback((): number[] => {
    if (!selection) return [];
    if (selection.rows.length > 0) {
      return selection.rows.toArray().filter((r) => r >= 0 && r < rowCount);
    }
    const cur = selection.current;
    if (!cur || columnCount <= 0) return [];
    const { x, y, width, height } = cur.range;
    const c1 = x + width - 1;
    const r1 = y + height - 1;
    if (x !== 0 || c1 < columnCount - 1) return [];
    const start = Math.max(0, y);
    const end = Math.min(rowCount - 1, r1);
    if (end < start) return [];
    const rows: number[] = [];
    for (let r = start; r <= end; r++) rows.push(r);
    return rows;
  }, [selection, columnCount, rowCount]);

  const selectAll = useCallback(() => {
    if (rowCount > 0 && columnCount > 0) {
      setSelection({
        columns: CompactSelection.empty(),
        rows: CompactSelection.empty(),
        current: {
          cell: [0, 0],
          range: { x: 0, y: 0, width: columnCount, height: rowCount },
          rangeStack: [],
        },
      });
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

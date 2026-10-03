import { useCallback, useState } from "react";
import {
  createSelectAllSelection,
  type DatabaseGridSelection,
} from "@/features/domain/databaseEditor/gridSelection";

interface UseSelectionParams {
  columnCount: number;
  rowCount: number;
}

export function useSelection({ columnCount, rowCount }: UseSelectionParams) {
  const [selection, setSelection] = useState<DatabaseGridSelection | null>(null);

  const selectAll = useCallback(() => {
    const nextSelection = createSelectAllSelection(columnCount, rowCount);
    if (nextSelection) setSelection(nextSelection);
  }, [columnCount, rowCount]);

  const clearSelection = useCallback(() => setSelection(null), []);

  return {
    selection,
    setSelection,
    selectAll,
    clearSelection,
  };
}

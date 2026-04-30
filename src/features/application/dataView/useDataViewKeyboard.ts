import { useEffect, useRef } from 'react';
import type { SelectionRange } from './useSelection';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import { isAppModalOpen } from '@/features/core/keyboard';

interface UseDataViewKeyboardParams {
  handleUndo: () => void;
  handleRedo: () => void;
  handleDeleteRow: (indices: number[]) => Promise<void>;
  selectAll: () => void;
  clearSelection: () => void;
  dismissContextMenu: () => void;
  selection: SelectionRange | null;
  selectedRowIndices: () => number[];
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement;
}

export function useDataViewKeyboard(params: UseDataViewKeyboardParams) {
  const paramsRef = useRef(params);
  paramsRef.current = params;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isAppModalOpen() || isTextEntryTarget(e.target)) {
        return;
      }

      const {
        handleUndo,
        handleRedo,
        handleDeleteRow,
        selectAll,
        clearSelection,
        dismissContextMenu,
        selection,
        selectedRowIndices,
      } = paramsRef.current;

      if (e.ctrlKey && e.key === 'z' && !e.shiftKey) {
        e.preventDefault(); handleUndo();
      } else if ((e.ctrlKey && e.shiftKey && e.key === 'Z') || (e.ctrlKey && e.key === 'y')) {
        e.preventDefault(); handleRedo();
      } else if (e.key === 'Escape') {
        dismissContextMenu(); clearSelection();
      } else if (e.ctrlKey && e.key === 'a') {
        e.preventDefault(); selectAll();
      } else if (e.key === 'Delete' && selection) {
        const rows = selectedRowIndices();
        if (rows.length > 0) { e.preventDefault(); handleDeleteRow(rows); clearSelection(); }
      }
    };
    return addGlobalEventListener(window, 'keydown', handler);
  }, []);
}

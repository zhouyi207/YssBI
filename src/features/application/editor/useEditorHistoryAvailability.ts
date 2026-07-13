import { useHistoryStore } from '@/features/core/history';
import type { GraphHistory } from '@/features/core/history';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';

/** Undo/redo availability for the focused editor group's active tab. */
export function useEditorHistoryAvailability() {
  const { activeTabId } = useActiveEditorGroup();

  const canUndo = useHistoryStore((s) => {
    if (!activeTabId) return false;
    const hist: GraphHistory | undefined = s.histories[activeTabId];
    return !!(hist && hist.undoStack.length > 0);
  });

  const canRedo = useHistoryStore((s) => {
    if (!activeTabId) return false;
    const hist: GraphHistory | undefined = s.histories[activeTabId];
    return !!(hist && hist.redoStack.length > 0);
  });

  return { canUndo, canRedo, activeTabId };
}

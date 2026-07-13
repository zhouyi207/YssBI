import type { HistoryEntry } from '@/features/core/history';
import type { FlatSidebarRow } from './types';
import { appendSectionBlock } from './appendSectionBlock';

export function buildCommandsFlatRows(params: {
  undoStack: HistoryEntry[];
  redoStack: HistoryEntry[];
  hasActiveTab: boolean;
  expandedSections: Record<string, boolean>;
  labels: {
    undo: string;
    redo: string;
    noHistory: string;
    noActiveGraph: string;
  };
}): FlatSidebarRow[] {
  const rows: FlatSidebarRow[] = [];
  const emptyMessage = params.hasActiveTab ? params.labels.noHistory : params.labels.noActiveGraph;

  const undoItems: FlatSidebarRow[] = [...params.undoStack].reverse().map((entry, index) => ({
    kind: 'history',
    rowKey: `history:undo:${entry.id}`,
    level: 1,
    entry,
    highlighted: index === 0,
    stack: 'undo',
  }));

  appendSectionBlock(rows, {
    sectionKey: 'commandsUndo',
    label: params.labels.undo,
    expandedSections: params.expandedSections,
    emptyMessage,
    itemRows: undoItems,
  });

  const redoItems: FlatSidebarRow[] = params.redoStack.map((entry) => ({
    kind: 'history',
    rowKey: `history:redo:${entry.id}`,
    level: 1,
    entry,
    highlighted: false,
    stack: 'redo',
  }));

  appendSectionBlock(rows, {
    sectionKey: 'commandsRedo',
    label: params.labels.redo,
    expandedSections: params.expandedSections,
    emptyMessage,
    itemRows: redoItems,
  });

  return rows;
}

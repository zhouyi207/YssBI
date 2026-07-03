/**
 * History Store — Command-based undo/redo state management.
 *
 * - Per-graph undo/redo stacks of HistoryEntry (command type + context)
 * - Merge logic for high-frequency operations (drag, typing)
 * - Undo/redo delegate to CommandHandler.undo/redo via the registry
 * - No snapshots — each entry stores only the minimal inverse context
 */

import { create } from 'zustand';
import { getCommandHandler } from './commands';
import { notifyStructuralChange } from './structuralChange';
import type {
  CommandType,
  ExecuteOptions,
  GraphHistory,
  HistoryEntry,
} from './types';
import { MAX_HISTORY, MERGE_WINDOW_MS } from '@/app/appConfig/default';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';

let entryCounter = 0;
function nextEntryId(): string {
  return `cmd_${++entryCounter}_${Date.now()}`;
}

function emptyHistory(): GraphHistory {
  return { undoStack: [], redoStack: [] };
}

interface HistoryStoreState {
  histories: Record<string, GraphHistory>;

  /** Push a completed command onto the undo stack (called by commandExecutor) */
  push: (
    graphId: string,
    commandType: CommandType,
    context: unknown,
    options?: ExecuteOptions,
  ) => void;

  /** Undo the last command for the given graph */
  undo: (graphId: string) => Promise<boolean>;

  /** Redo the last undone command for the given graph */
  redo: (graphId: string) => Promise<boolean>;

  canUndo: (graphId: string) => boolean;
  canRedo: (graphId: string) => boolean;

  /** Clear history for a specific graph or all graphs */
  clear: (graphId?: string) => void;
}

export const useHistoryStore = create<HistoryStoreState>((set, get) => ({
  histories: {},

  push: (graphId, commandType, context, options) => {
    const now = Date.now();
    const mergeKey = options?.mergeKey;

    set((state) => {
      const hist = state.histories[graphId] ?? emptyHistory();
      const stack = hist.undoStack;
      const top = stack[stack.length - 1];

      if (
        mergeKey &&
        top?.mergeKey === mergeKey &&
        now - top.timestamp < MERGE_WINDOW_MS
      ) {
        const updated = [...stack];
        updated[updated.length - 1] = { ...top, context, timestamp: now };
        return {
          histories: {
            ...state.histories,
            [graphId]: { undoStack: updated, redoStack: [] },
          },
        };
      }

      const entry: HistoryEntry = {
        id: nextEntryId(),
        graphId,
        commandType,
        context,
        timestamp: now,
        mergeKey,
      };

      return {
        histories: {
          ...state.histories,
          [graphId]: {
            undoStack: [...stack, entry].slice(-MAX_HISTORY),
            redoStack: [],
          },
        },
      };
    });
  },

  undo: async (graphId) => {
    const hist = get().histories[graphId];
    if (!hist || hist.undoStack.length === 0) return false;

    const entry = hist.undoStack[hist.undoStack.length - 1];

    try {
      const handler = getCommandHandler(entry.commandType);
      await handler.undo(graphId, entry.context);
      notifyStructuralChange(entry.commandType, graphId);
      set((state) => {
        const h = state.histories[graphId]!;
        return {
          histories: {
            ...state.histories,
            [graphId]: {
              undoStack: h.undoStack.slice(0, -1),
              redoStack: [entry, ...h.redoStack].slice(0, MAX_HISTORY),
            },
          },
        };
      });
      return true;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      logger.graph.error(`Undo failed for ${entry.commandType}: ${msg}`, 'HistoryStore');
      uiStore.showToast(`撤销失败：${msg}`, 'error', 3000);
      return false;
    }
  },

  redo: async (graphId) => {
    const hist = get().histories[graphId];
    if (!hist || hist.redoStack.length === 0) return false;

    const entry = hist.redoStack[0];

    try {
      const handler = getCommandHandler(entry.commandType);
      await handler.redo(graphId, entry.context);
      notifyStructuralChange(entry.commandType, graphId);
      set((state) => {
        const h = state.histories[graphId]!;
        return {
          histories: {
            ...state.histories,
            [graphId]: {
              undoStack: [...h.undoStack, entry].slice(-MAX_HISTORY),
              redoStack: h.redoStack.slice(1),
            },
          },
        };
      });
      return true;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      logger.graph.error(`Redo failed for ${entry.commandType}: ${msg}`, 'HistoryStore');
      uiStore.showToast(`重做失败：${msg}`, 'error', 3000);
      return false;
    }
  },

  canUndo: (graphId) => {
    const hist = get().histories[graphId];
    return !!(hist && hist.undoStack.length > 0);
  },

  canRedo: (graphId) => {
    const hist = get().histories[graphId];
    return !!(hist && hist.redoStack.length > 0);
  },

  clear: (graphId) => {
    if (graphId) {
      set((state) => {
        const { [graphId]: _, ...rest } = state.histories;
        return { histories: rest };
      });
    } else {
      set({ histories: {} });
    }
  },
}));

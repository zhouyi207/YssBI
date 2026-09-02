import { create } from "zustand";

import type {
  EditorGraphProjectionDto,
  GraphDocumentDto,
  GraphDraftSaveDto,
  GraphDraftUpdateDto,
  GraphEditorSessionDto,
} from "@/shared/types/domain/editorMutation";

interface GraphDraftVersion {
  readonly document: GraphDocumentDto;
  readonly projection: EditorGraphProjectionDto;
}

export interface GraphDraftSession extends GraphDraftVersion {
  readonly saving: boolean;
  readonly savedDocument: GraphDocumentDto;
  readonly undoStack: readonly GraphDraftVersion[];
  readonly redoStack: readonly GraphDraftVersion[];
}

interface GraphDraftStore {
  readonly sessions: Readonly<Record<string, GraphDraftSession>>;
  install(graphPath: string, session: GraphEditorSessionDto): void;
  applyUpdate(graphPath: string, update: GraphDraftUpdateDto): void;
  beginSave(graphPath: string): boolean;
  completeSave(graphPath: string, saved: GraphDraftSaveDto): void;
  failSave(graphPath: string): void;
  undo(graphPath: string): EditorGraphProjectionDto | null;
  redo(graphPath: string): EditorGraphProjectionDto | null;
  clearGraph(graphPath: string): void;
  clear(): void;
}

function cloneVersion(version: GraphDraftVersion): GraphDraftVersion {
  return {
    document: structuredClone(version.document),
    projection: structuredClone(version.projection),
  };
}

export const useGraphDraftStore = create<GraphDraftStore>((set, get) => ({
  sessions: {},

  install: (graphPath, session) =>
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          document: structuredClone(session.document),
          projection: structuredClone(session.projection),
          saving: false,
          savedDocument: structuredClone(session.document),
          undoStack: [],
          redoStack: [],
        },
      },
    })),

  applyUpdate: (graphPath, update) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current) throw new Error(`Graph draft '${graphPath}' is not loaded`);
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            document: structuredClone(update.document),
            projection: structuredClone(update.projectionReplacement.projection),
            saving: current.saving,
            savedDocument: current.savedDocument,
            undoStack: [...current.undoStack, cloneVersion(current)],
            redoStack: [],
          },
        },
      };
    }),

  beginSave: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving) return false;
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: { ...state.sessions[graphPath], saving: true },
      },
    }));
    return true;
  },

  completeSave: (graphPath, saved) =>
    set((state) => {
      if (!state.sessions[graphPath]) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            document: structuredClone(saved.document),
            projection: structuredClone(saved.projectionReplacement.projection),
            saving: false,
            savedDocument: structuredClone(saved.document),
            undoStack: [],
            redoStack: [],
          },
        },
      };
    }),

  failSave: (graphPath) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current?.saving) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: { ...current, saving: false },
        },
      };
    }),

  undo: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.undoStack.length === 0) return null;
    const previous = current.undoStack[current.undoStack.length - 1];
    const nextUndo = current.undoStack.slice(0, -1);
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...cloneVersion(previous),
          saving: false,
          savedDocument: current.savedDocument,
          undoStack: nextUndo,
          redoStack: [...current.redoStack, cloneVersion(current)],
        },
      },
    }));
    return structuredClone(previous.projection);
  },

  redo: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.redoStack.length === 0) return null;
    const next = current.redoStack[current.redoStack.length - 1];
    const nextRedo = current.redoStack.slice(0, -1);
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...cloneVersion(next),
          saving: false,
          savedDocument: current.savedDocument,
          undoStack: [...current.undoStack, cloneVersion(current)],
          redoStack: nextRedo,
        },
      },
    }));
    return structuredClone(next.projection);
  },

  clearGraph: (graphPath) =>
    set((state) => {
      if (!state.sessions[graphPath]) return state;
      const sessions = { ...state.sessions };
      delete sessions[graphPath];
      return { sessions };
    }),

  clear: () => set({ sessions: {} }),
}));

export function getGraphDraftDocument(graphPath: string): GraphDocumentDto | null {
  const document = useGraphDraftStore.getState().sessions[graphPath]?.document;
  return document ? structuredClone(document) : null;
}

export function isGraphDraftSaving(graphPath: string): boolean {
  return useGraphDraftStore.getState().sessions[graphPath]?.saving === true;
}

export function isGraphDraftDirty(graphPath: string): boolean {
  const session = useGraphDraftStore.getState().sessions[graphPath];
  return (
    Boolean(session) && JSON.stringify(session.document) !== JSON.stringify(session.savedDocument)
  );
}

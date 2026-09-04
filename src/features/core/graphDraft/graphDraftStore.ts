import { create } from "zustand";

import type {
  EditorGraphProjectionDto,
  GraphDocumentDto,
  CompileGraphDraftDto,
  GraphDraftSaveDto,
  GraphDraftAcceptedDto,
  GraphEditorSessionDto,
} from "@/shared/types/domain/editorMutation";

interface GraphDraftVersion {
  readonly document: GraphDocumentDto;
  readonly projection: EditorGraphProjectionDto;
}

export interface GraphDraftSession extends GraphDraftVersion {
  readonly draftRevision: number;
  readonly saving: boolean;
  readonly compileStatus: "uncompiled" | "compiling" | "compiled" | "failed";
  readonly compiledSourceHash: string | null;
  readonly compileCacheHit: boolean;
  readonly savedDocument: GraphDocumentDto;
  readonly undoStack: readonly GraphDraftVersion[];
  readonly redoStack: readonly GraphDraftVersion[];
}

interface GraphDraftStore {
  readonly sessions: Readonly<Record<string, GraphDraftSession>>;
  install(graphPath: string, session: GraphEditorSessionDto): void;
  applyAcceptedUpdate(
    graphPath: string,
    update: GraphDraftAcceptedDto,
    projection: EditorGraphProjectionDto,
  ): void;
  acceptNoop(graphPath: string, acceptedRevision: number): void;
  beginCompile(graphPath: string): boolean;
  completeCompile(graphPath: string, result: CompileGraphDraftDto): void;
  failCompile(graphPath: string): void;
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
          draftRevision: 0,
          saving: false,
          compileStatus: "uncompiled",
          compiledSourceHash: null,
          compileCacheHit: false,
          savedDocument: structuredClone(session.document),
          undoStack: [],
          redoStack: [],
        },
      },
    })),

  applyAcceptedUpdate: (graphPath, update, projection) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current) throw new Error(`Graph draft '${graphPath}' is not loaded`);
      if (update.acceptedRevision !== current.draftRevision + 1) {
        throw new Error(`Graph draft '${graphPath}' acceptance revision is not monotonic`);
      }
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            document: structuredClone(update.document),
            projection: structuredClone(projection),
            draftRevision: update.acceptedRevision,
            saving: current.saving,
            compileStatus: "uncompiled",
            compiledSourceHash: null,
            compileCacheHit: false,
            savedDocument: current.savedDocument,
            undoStack: [...current.undoStack, cloneVersion(current)],
            redoStack: [],
          },
        },
      };
    }),

  acceptNoop: (graphPath, acceptedRevision) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current) throw new Error(`Graph draft '${graphPath}' is not loaded`);
      if (acceptedRevision !== current.draftRevision + 1) {
        throw new Error(`Graph draft '${graphPath}' acceptance revision is not monotonic`);
      }
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: { ...current, draftRevision: acceptedRevision },
        },
      };
    }),

  beginCompile: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.compileStatus === "compiling") return false;
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: { ...state.sessions[graphPath], compileStatus: "compiling" },
      },
    }));
    return true;
  },

  completeCompile: (graphPath, result) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current || current.compileStatus !== "compiling") return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            document: structuredClone(result.document),
            projection: structuredClone(result.projection),
            compileStatus: "compiled",
            compiledSourceHash: result.sourceHash,
            compileCacheHit: result.cacheHit,
          },
        },
      };
    }),

  failCompile: (graphPath) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current || current.compileStatus !== "compiling") return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            compileStatus: "failed",
            compiledSourceHash: null,
            compileCacheHit: false,
          },
        },
      };
    }),

  beginSave: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.compileStatus === "compiling") return false;
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
      const current = state.sessions[graphPath];
      if (!current) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            document: structuredClone(saved.document),
            projection: structuredClone(saved.projectionReplacement.projection),
            draftRevision: current.draftRevision,
            saving: false,
            compileStatus: current.compileStatus,
            compiledSourceHash: current.compiledSourceHash,
            compileCacheHit: current.compileCacheHit,
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
          draftRevision: current.draftRevision + 1,
          saving: false,
          compileStatus: "uncompiled",
          compiledSourceHash: null,
          compileCacheHit: false,
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
          draftRevision: current.draftRevision + 1,
          saving: false,
          compileStatus: "uncompiled",
          compiledSourceHash: null,
          compileCacheHit: false,
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

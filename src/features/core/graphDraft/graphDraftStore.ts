import { create } from "zustand";

import type {
  EditorGraphProjectionDto,
  GraphDocumentDto,
  CompileGraphDraftDto,
  GraphDraftSaveDto,
  GraphDraftTransformDto,
  GraphEditorSessionDto,
} from "@/shared/types/domain/editorMutation";

interface GraphDraftVersion {
  readonly document: GraphDocumentDto;
  readonly projection: EditorGraphProjectionDto;
}

export interface GraphDraftSession extends GraphDraftVersion {
  readonly sessionId: number;
  readonly draftGeneration: number;
  readonly semanticInputHash: string;
  readonly compiledInputHash: string | null;
  readonly compileRequest: GraphCompileRequest | null;
  readonly saveDirty: boolean;
  readonly compileDirty: boolean;
  readonly saving: boolean;
  readonly compileStatus: "uncompiled" | "compiling" | "compiled" | "blocked" | "failed";
  readonly compiledArtifactId: string | null;
  readonly compileCacheHit: boolean;
  readonly savedDocument: GraphDocumentDto;
  readonly undoStack: readonly GraphDraftVersion[];
  readonly redoStack: readonly GraphDraftVersion[];
}

export interface GraphCompileRequest {
  readonly sessionId: number;
  readonly draftGeneration: number;
  readonly requestId: number;
}

let nextSessionId = 0;
let nextCompileRequestId = 0;

function isCurrentCompile(
  session: GraphDraftSession | undefined,
  request: GraphCompileRequest,
): boolean {
  return Boolean(
    session &&
    session.sessionId === request.sessionId &&
    session.draftGeneration === request.draftGeneration &&
    session.compileRequest?.requestId === request.requestId,
  );
}

function editedCompileState(current: GraphDraftSession, projection: EditorGraphProjectionDto) {
  const semanticInputHash = projection.basis.semanticInputHash;
  const matchesArtifact =
    current.compiledArtifactId !== null && current.compiledInputHash === semanticInputHash;
  return {
    draftGeneration: current.draftGeneration + 1,
    semanticInputHash,
    compiledInputHash: matchesArtifact ? current.compiledInputHash : null,
    compiledArtifactId: matchesArtifact ? current.compiledArtifactId : null,
    compileCacheHit: matchesArtifact && current.compileCacheHit,
    compileRequest: null,
    compileStatus: matchesArtifact ? ("compiled" as const) : ("uncompiled" as const),
    compileDirty: !matchesArtifact,
  };
}

interface GraphDraftStore {
  readonly sessions: Readonly<Record<string, GraphDraftSession>>;
  install(graphPath: string, session: GraphEditorSessionDto): void;
  hydrate(graphPath: string, session: GraphEditorSessionDto): void;
  replaceResolvedProjection(graphPath: string, projection: EditorGraphProjectionDto): void;
  applyTransform(graphPath: string, update: GraphDraftTransformDto): void;
  beginCompile(graphPath: string): boolean;
  isCompileCurrent(graphPath: string, request: GraphCompileRequest): boolean;
  completeCompile(
    graphPath: string,
    result: CompileGraphDraftDto,
    request: GraphCompileRequest,
  ): void;
  failCompile(graphPath: string, request: GraphCompileRequest): void;
  beginSave(graphPath: string): boolean;
  completeSave(graphPath: string, saved: GraphDraftSaveDto): void;
  failSave(graphPath: string): void;
  undo(graphPath: string, projection: EditorGraphProjectionDto): EditorGraphProjectionDto | null;
  redo(graphPath: string, projection: EditorGraphProjectionDto): EditorGraphProjectionDto | null;
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
          sessionId: ++nextSessionId,
          draftGeneration: 0,
          semanticInputHash: session.projection.basis.semanticInputHash,
          compiledInputHash: null,
          compileRequest: null,
          saveDirty: false,
          compileDirty: true,
          document: structuredClone(session.document),
          projection: structuredClone(session.projection),
          saving: false,
          compileStatus: "uncompiled",
          compiledArtifactId: null,
          compileCacheHit: false,
          savedDocument: structuredClone(session.document),
          undoStack: [],
          redoStack: [],
        },
      },
    })),

  hydrate: (graphPath, session) => {
    const current = get().sessions[graphPath];
    if (!current) {
      get().install(graphPath, session);
      return;
    }
    if (current.saveDirty || current.saving) return;
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...current,
          ...cloneVersion(session),
          ...editedCompileState(current, session.projection),
          savedDocument: structuredClone(session.document),
          saveDirty: false,
          undoStack: [],
          redoStack: [],
        },
      },
    }));
  },

  replaceResolvedProjection: (graphPath, projection) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current || current.saving) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            ...editedCompileState(current, projection),
            projection: structuredClone(projection),
          },
        },
      };
    }),

  applyTransform: (graphPath, update) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current) throw new Error(`Graph draft '${graphPath}' is not loaded`);
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            ...editedCompileState(current, update.projection),
            saveDirty: JSON.stringify(update.document) !== JSON.stringify(current.savedDocument),
            document: structuredClone(update.document),
            projection: structuredClone(update.projection),
            saving: current.saving,
            savedDocument: current.savedDocument,
            undoStack: [...current.undoStack, cloneVersion(current)],
            redoStack: [],
          },
        },
      };
    }),

  beginCompile: (graphPath) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.compileStatus === "compiling") return false;
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...state.sessions[graphPath],
          compileStatus: "compiling",
          compileRequest: {
            sessionId: current.sessionId,
            draftGeneration: current.draftGeneration,
            requestId: ++nextCompileRequestId,
          },
        },
      },
    }));
    return true;
  },

  isCompileCurrent: (graphPath, request) => isCurrentCompile(get().sessions[graphPath], request),

  completeCompile: (graphPath, result, request) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current || !isCurrentCompile(current, request)) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            compileRequest: null,
            semanticInputHash: result.projection.basis.semanticInputHash,
            compiledInputHash:
              result.type === "ready" ? result.projection.basis.semanticInputHash : null,
            compileDirty: result.type !== "ready",
            projection: structuredClone(result.projection),
            compileStatus: result.type === "ready" ? "compiled" : "blocked",
            compiledArtifactId: result.type === "ready" ? result.artifactId : null,
            compileCacheHit: result.type === "ready" && result.cacheHit,
          },
        },
      };
    }),

  failCompile: (graphPath, request) =>
    set((state) => {
      const current = state.sessions[graphPath];
      if (!current || !isCurrentCompile(current, request)) return state;
      return {
        sessions: {
          ...state.sessions,
          [graphPath]: {
            ...current,
            compileStatus: "failed",
            compileRequest: null,
            compiledInputHash: null,
            compileDirty: true,
            compiledArtifactId: null,
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
            ...current,
            ...editedCompileState(current, saved.projectionReplacement.projection),
            saveDirty: false,
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

  undo: (graphPath, projection) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.undoStack.length === 0) return null;
    const previous = { ...current.undoStack[current.undoStack.length - 1], projection };
    const nextUndo = current.undoStack.slice(0, -1);
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...cloneVersion(previous),
          sessionId: current.sessionId,
          ...editedCompileState(current, previous.projection),
          saveDirty: JSON.stringify(previous.document) !== JSON.stringify(current.savedDocument),
          saving: false,
          savedDocument: current.savedDocument,
          undoStack: nextUndo,
          redoStack: [...current.redoStack, cloneVersion(current)],
        },
      },
    }));
    return structuredClone(previous.projection);
  },

  redo: (graphPath, projection) => {
    const current = get().sessions[graphPath];
    if (!current || current.saving || current.redoStack.length === 0) return null;
    const next = { ...current.redoStack[current.redoStack.length - 1], projection };
    const nextRedo = current.redoStack.slice(0, -1);
    set((state) => ({
      sessions: {
        ...state.sessions,
        [graphPath]: {
          ...cloneVersion(next),
          sessionId: current.sessionId,
          ...editedCompileState(current, next.projection),
          saveDirty: JSON.stringify(next.document) !== JSON.stringify(current.savedDocument),
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
  return session?.saveDirty ?? false;
}

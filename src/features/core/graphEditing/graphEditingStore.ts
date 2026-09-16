import { create } from "zustand";
import type {
  EditorGraphProjectionDto,
  GraphDocumentDto,
  GraphEditorSessionDto,
  GraphEditVersionDto,
} from "@/shared/types/domain/editorMutation";

/** Read projection of the current Rust-owned graph; history and saved content remain in Rust. */
export interface GraphEditorState {
  readonly document: GraphDocumentDto;
  readonly projection: EditorGraphProjectionDto;
  readonly version: GraphEditVersionDto;
  readonly sessionId: number;
  readonly projectionGeneration: number;
  readonly semanticInputHash: string;
  readonly saveDirty: boolean;
  readonly saving: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
}

interface GraphEditingStore {
  readonly sessions: Readonly<Record<string, GraphEditorState>>;
  install(graphPath: string, session: GraphEditorSessionDto): void;
  hydrate(graphPath: string, session: GraphEditorSessionDto, saving?: boolean): void;
  beginSave(graphPath: string): boolean;
  failSave(graphPath: string): void;
  clearGraph(graphPath: string): void;
  clear(): void;
}

export function canAcceptGraphSession(
  previous: GraphEditorState | undefined,
  input: GraphEditorSessionDto,
): boolean {
  return (
    previous?.version.sessionId !== input.editing.version.sessionId ||
    BigInt(previous.version.revision) <= BigInt(input.editing.version.revision)
  );
}

let nextViewSessionId = 0;

function installSession(
  state: GraphEditingStore,
  graphPath: string,
  input: GraphEditorSessionDto,
  saving?: boolean,
  renew = false,
): Partial<GraphEditingStore> | GraphEditingStore {
  const previous = state.sessions[graphPath];
  if (!canAcceptGraphSession(previous, input)) return state;
  if (
    !renew &&
    previous &&
    previous.document === input.document &&
    previous.projection === input.projection &&
    previous.version.sessionId === input.editing.version.sessionId &&
    previous.version.revision === input.editing.version.revision &&
    previous.saveDirty === input.editing.dirty &&
    previous.canUndo === input.editing.canUndo &&
    previous.canRedo === input.editing.canRedo &&
    (saving === undefined || saving === previous.saving)
  )
    return state;
  return {
    sessions: {
      ...state.sessions,
      [graphPath]: {
        document: input.document,
        projection: input.projection,
        version: input.editing.version,
        sessionId:
          !renew && previous?.version.sessionId === input.editing.version.sessionId
            ? previous.sessionId
            : ++nextViewSessionId,
        projectionGeneration: (previous?.projectionGeneration ?? 0) + 1,
        semanticInputHash: input.projection.basis.semanticInputHash,
        saveDirty: input.editing.dirty,
        canUndo: input.editing.canUndo,
        canRedo: input.editing.canRedo,
        saving: saving ?? previous?.saving ?? false,
      },
    },
  };
}

export const useGraphEditingStore = create<GraphEditingStore>((set, get) => ({
  sessions: {},
  install: (path, input) => set((state) => installSession(state, path, input, false, true)),
  hydrate: (path, input, saving) => set((state) => installSession(state, path, input, saving)),
  beginSave: (path) => {
    const current = get().sessions[path];
    if (!current || current.saving) return false;
    set((state) => ({ sessions: { ...state.sessions, [path]: { ...current, saving: true } } }));
    return true;
  },
  failSave: (path) =>
    set((state) => {
      const current = state.sessions[path];
      return current
        ? { sessions: { ...state.sessions, [path]: { ...current, saving: false } } }
        : state;
    }),
  clearGraph: (path) =>
    set((state) => {
      const sessions = { ...state.sessions };
      delete sessions[path];
      return { sessions };
    }),
  clear: () => set({ sessions: {} }),
}));

export function getGraphDocumentProjection(graphPath: string): GraphDocumentDto | null {
  return useGraphEditingStore.getState().sessions[graphPath]?.document ?? null;
}
export function isGraphSaving(graphPath: string): boolean {
  return useGraphEditingStore.getState().sessions[graphPath]?.saving === true;
}
export function isGraphModified(graphPath: string): boolean {
  return useGraphEditingStore.getState().sessions[graphPath]?.saveDirty === true;
}

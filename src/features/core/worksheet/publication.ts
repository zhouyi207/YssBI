import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import type {
  PendingWorksheetSave,
  WorksheetCommittedSnapshot,
  WorksheetReadSnapshot,
} from './read';

export interface OptimisticOperationKey {
  readonly projectInstanceId: string;
  readonly resourceKey: string;
  readonly operationId: string;
  readonly fromRevision: number;
}

export interface WorksheetProjectionPublication {
  replaceSnapshot(snapshot: DeepReadonly<WorksheetCommittedSnapshot>): void;
  applyCommittedDocument(
    worksheetPath: string,
    document: DeepReadonly<WorksheetDocument>,
  ): void;
  removeCommittedDocument(worksheetPath: string): void;
  clearForProject(projectInstanceId: string | null): void;
}

export interface WorksheetSavePublication {
  beginPendingSave(record: PendingWorksheetSave): void;
  markPendingSaveAcknowledged(key: OptimisticOperationKey): void;
  markPendingSaveUnknown(key: OptimisticOperationKey): void;
  settlePendingSave(key: OptimisticOperationKey): void;
  clearPendingSaves(projectInstanceId: string | null): void;
}

interface MutableWorksheetSnapshot {
  index: WorksheetIndexEntry[];
  documents: Record<string, WorksheetDocument>;
  draftsByPath: Record<string, WorksheetDocument>;
  dirtyByPath: Record<string, boolean>;
  pendingSaveByPath: Record<string, Record<string, PendingWorksheetSave>>;
}

const EMPTY_SNAPSHOT: WorksheetReadSnapshot = {
  index: [],
  documents: {},
  draftsByPath: {},
  dirtyByPath: {},
  pendingSaveByPath: {},
};

let currentSnapshot: DeepReadonly<WorksheetReadSnapshot> = freezeSnapshot(EMPTY_SNAPSHOT);
let activeProjectInstanceId: string | null = null;
const listeners = new Set<() => void>();

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneValue(nested)]),
  ) as T;
}

function freezeValue<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(freezeValue)) as T;
  if (value === null || typeof value !== 'object') return value;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, freezeValue(nested)]),
  )) as T;
}

function freezeSnapshot(value: WorksheetReadSnapshot): DeepReadonly<WorksheetReadSnapshot> {
  return freezeValue({
    index: value.index,
    documents: value.documents,
    draftsByPath: value.draftsByPath,
    dirtyByPath: value.dirtyByPath,
    pendingSaveByPath: value.pendingSaveByPath,
  });
}

function mutableSnapshot(): MutableWorksheetSnapshot {
  return {
    index: cloneValue(currentSnapshot.index) as WorksheetIndexEntry[],
    documents: cloneValue(currentSnapshot.documents) as Record<string, WorksheetDocument>,
    draftsByPath: cloneValue(currentSnapshot.draftsByPath) as Record<string, WorksheetDocument>,
    dirtyByPath: cloneValue(currentSnapshot.dirtyByPath) as Record<string, boolean>,
    pendingSaveByPath: cloneValue(currentSnapshot.pendingSaveByPath) as Record<string, Record<string, PendingWorksheetSave>>,
  };
}

function emptyMutableSnapshot(): MutableWorksheetSnapshot {
  return {
    index: [],
    documents: {},
    draftsByPath: {},
    dirtyByPath: {},
    pendingSaveByPath: {},
  };
}

function publish(next: MutableWorksheetSnapshot): void {
  currentSnapshot = freezeSnapshot(next);
  for (const listener of listeners) listener();
}

function projectMatches(projectInstanceId: string): boolean {
  return activeProjectInstanceId === null || activeProjectInstanceId === projectInstanceId;
}

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, stableValue(nested)]),
  );
}

export function worksheetDocumentFingerprint(
  document: DeepReadonly<WorksheetDocument>,
): string {
  return JSON.stringify(stableValue(document));
}

export function optimisticOperationKey(key: OptimisticOperationKey): string {
  return JSON.stringify([
    key.projectInstanceId,
    key.resourceKey,
    key.operationId,
    key.fromRevision,
  ]);
}

export function getWorksheetSnapshot(): DeepReadonly<WorksheetReadSnapshot> {
  return currentSnapshot;
}

export function subscribeWorksheetRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function findPendingWorksheetSave(
  key: OptimisticOperationKey,
): DeepReadonly<PendingWorksheetSave> | undefined {
  return currentSnapshot.pendingSaveByPath[key.resourceKey]?.[optimisticOperationKey(key)];
}

export function applyWorksheetDraftUpdate(
  worksheetPath: string,
  patch: DeepReadonly<Partial<WorksheetDocument>>,
): DeepReadonly<WorksheetDocument> | null {
  const current = currentSnapshot.draftsByPath[worksheetPath]
    ?? currentSnapshot.documents[worksheetPath];
  if (!current) return null;

  const next: WorksheetDocument = {
    ...cloneValue(current),
    ...cloneValue(patch),
    encodings: {
      ...cloneValue(current.encodings),
      ...(patch.encodings ? cloneValue(patch.encodings) : {}),
    },
  };
  const state = mutableSnapshot();
  state.draftsByPath[worksheetPath] = next;
  const committed = state.documents[worksheetPath];
  state.dirtyByPath[worksheetPath] = committed === undefined
    || worksheetDocumentFingerprint(next) !== worksheetDocumentFingerprint(committed);
  publish(state);
  return currentSnapshot.draftsByPath[worksheetPath] ?? null;
}

export function discardWorksheetDraft(worksheetPath: string): void {
  const state = mutableSnapshot();
  delete state.draftsByPath[worksheetPath];
  state.dirtyByPath[worksheetPath] = false;
  publish(state);
}

export function rebaseWorksheetDraft(
  worksheetPath: string,
  _committed: DeepReadonly<WorksheetDocument>,
  expectedDraftFingerprint: string,
): 'rebased' | 'draft-changed' {
  const currentDraft = currentSnapshot.draftsByPath[worksheetPath];
  if (!currentDraft
    || worksheetDocumentFingerprint(currentDraft) !== expectedDraftFingerprint) {
    return 'draft-changed';
  }

  const state = mutableSnapshot();
  delete state.draftsByPath[worksheetPath];
  state.dirtyByPath[worksheetPath] = false;
  publish(state);
  return 'rebased';
}

function updatePendingSaveStatus(
  key: OptimisticOperationKey,
  status: PendingWorksheetSave['status'],
): void {
  if (!projectMatches(key.projectInstanceId)) return;
  const operationKey = optimisticOperationKey(key);
  const existing = currentSnapshot.pendingSaveByPath[key.resourceKey]?.[operationKey];
  if (!existing) return;
  const state = mutableSnapshot();
  state.pendingSaveByPath[key.resourceKey][operationKey] = {
    ...existing,
    status,
  };
  publish(state);
}

export const worksheetProjectionPublication: WorksheetProjectionPublication = {
  replaceSnapshot: (snapshot) => {
    const state = mutableSnapshot();
    state.index = cloneValue(snapshot.index) as WorksheetIndexEntry[];
    state.documents = cloneValue(snapshot.documents) as Record<string, WorksheetDocument>;
    publish(state);
  },

  applyCommittedDocument: (worksheetPath, document) => {
    const state = mutableSnapshot();
    state.documents[worksheetPath] = cloneValue(document) as WorksheetDocument;
    publish(state);
  },

  removeCommittedDocument: (worksheetPath) => {
    const state = mutableSnapshot();
    delete state.documents[worksheetPath];
    state.index = state.index.filter((entry) => entry.worksheetPath !== worksheetPath);
    publish(state);
  },

  clearForProject: (projectInstanceId) => {
    activeProjectInstanceId = projectInstanceId;
    publish(emptyMutableSnapshot());
  },
};

export const worksheetSavePublication: WorksheetSavePublication = {
  beginPendingSave: (record) => {
    if (!projectMatches(record.projectInstanceId)) return;
    const operationKey = optimisticOperationKey(record);
    const state = mutableSnapshot();
    const records = state.pendingSaveByPath[record.resourceKey] ?? {};
    state.pendingSaveByPath[record.resourceKey] = {
      ...records,
      [operationKey]: cloneValue(record),
    };
    publish(state);
  },

  markPendingSaveAcknowledged: (key) => updatePendingSaveStatus(key, 'acknowledged'),

  markPendingSaveUnknown: (key) => updatePendingSaveStatus(key, 'unknown'),

  settlePendingSave: (key) => {
    if (!projectMatches(key.projectInstanceId)) return;
    const operationKey = optimisticOperationKey(key);
    const records = currentSnapshot.pendingSaveByPath[key.resourceKey];
    if (!records?.[operationKey]) return;
    const state = mutableSnapshot();
    const nextRecords = { ...state.pendingSaveByPath[key.resourceKey] };
    delete nextRecords[operationKey];
    if (Object.keys(nextRecords).length === 0) {
      delete state.pendingSaveByPath[key.resourceKey];
    } else {
      state.pendingSaveByPath[key.resourceKey] = nextRecords;
    }
    publish(state);
  },

  clearPendingSaves: (projectInstanceId) => {
    if (projectInstanceId === null) {
      const state = mutableSnapshot();
      state.pendingSaveByPath = {};
      publish(state);
      return;
    }

    const state = mutableSnapshot();
    for (const [path, records] of Object.entries(state.pendingSaveByPath)) {
      const retained = Object.fromEntries(
        Object.entries(records).filter(([, record]) => record.projectInstanceId !== projectInstanceId),
      );
      if (Object.keys(retained).length === 0) delete state.pendingSaveByPath[path];
      else state.pendingSaveByPath[path] = retained;
    }
    publish(state);
  },
};

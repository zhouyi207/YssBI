import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument, ChartIndexEntry } from "@/shared/types/domain/chart";
import type { PendingChartSave, ChartCommittedSnapshot, ChartReadSnapshot } from "./read";

export interface OptimisticOperationKey {
  readonly projectInstanceId: string;
  readonly resourceKey: string;
  readonly operationId: string;
  readonly fromRevision: number;
}

export interface ChartProjectionPublication {
  replaceSnapshot(snapshot: DeepReadonly<ChartCommittedSnapshot>): void;
  applyCommittedDocument(chartPath: string, document: DeepReadonly<ChartDocument>): void;
  removeCommittedDocument(chartPath: string): void;
  clearForProject(projectInstanceId: string | null): void;
}

export interface ChartSavePublication {
  beginPendingSave(record: PendingChartSave): void;
  markPendingSaveAcknowledged(key: OptimisticOperationKey): void;
  markPendingSaveUnknown(key: OptimisticOperationKey): void;
  settlePendingSave(key: OptimisticOperationKey): void;
  clearPendingSaves(projectInstanceId: string | null): void;
}

interface MutableChartSnapshot {
  index: ChartIndexEntry[];
  documents: Record<string, ChartDocument>;
  draftsByPath: Record<string, ChartDocument>;
  dirtyByPath: Record<string, boolean>;
  pendingSaveByPath: Record<string, Record<string, PendingChartSave>>;
}

const EMPTY_SNAPSHOT: ChartReadSnapshot = {
  index: [],
  documents: {},
  draftsByPath: {},
  dirtyByPath: {},
  pendingSaveByPath: {},
};

let currentSnapshot: DeepReadonly<ChartReadSnapshot> = freezeSnapshot(EMPTY_SNAPSHOT);
let activeProjectInstanceId: string | null = null;
const listeners = new Set<() => void>();

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
      key,
      cloneValue(nested),
    ]),
  ) as T;
}

function freezeValue<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(freezeValue)) as T;
  if (value === null || typeof value !== "object") return value;
  return Object.freeze(
    Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
        key,
        freezeValue(nested),
      ]),
    ),
  ) as T;
}

function freezeSnapshot(value: ChartReadSnapshot): DeepReadonly<ChartReadSnapshot> {
  return freezeValue({
    index: value.index,
    documents: value.documents,
    draftsByPath: value.draftsByPath,
    dirtyByPath: value.dirtyByPath,
    pendingSaveByPath: value.pendingSaveByPath,
  });
}

function mutableSnapshot(): MutableChartSnapshot {
  return {
    index: cloneValue(currentSnapshot.index) as ChartIndexEntry[],
    documents: cloneValue(currentSnapshot.documents) as Record<string, ChartDocument>,
    draftsByPath: cloneValue(currentSnapshot.draftsByPath) as Record<string, ChartDocument>,
    dirtyByPath: cloneValue(currentSnapshot.dirtyByPath) as Record<string, boolean>,
    pendingSaveByPath: cloneValue(currentSnapshot.pendingSaveByPath) as Record<
      string,
      Record<string, PendingChartSave>
    >,
  };
}

function emptyMutableSnapshot(): MutableChartSnapshot {
  return {
    index: [],
    documents: {},
    draftsByPath: {},
    dirtyByPath: {},
    pendingSaveByPath: {},
  };
}

function publish(next: MutableChartSnapshot): void {
  currentSnapshot = freezeSnapshot(next);
  for (const listener of listeners) listener();
}

function projectMatches(projectInstanceId: string): boolean {
  return activeProjectInstanceId === null || activeProjectInstanceId === projectInstanceId;
}

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, stableValue(nested)]),
  );
}

export function chartDocumentFingerprint(document: DeepReadonly<ChartDocument>): string {
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

export function getChartSnapshot(): DeepReadonly<ChartReadSnapshot> {
  return currentSnapshot;
}

export function subscribeChartRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function findPendingChartSave(
  key: OptimisticOperationKey,
): DeepReadonly<PendingChartSave> | undefined {
  return currentSnapshot.pendingSaveByPath[key.resourceKey]?.[optimisticOperationKey(key)];
}

export function applyChartDraftUpdate(
  chartPath: string,
  patch: DeepReadonly<Partial<ChartDocument>>,
): DeepReadonly<ChartDocument> | null {
  const current = currentSnapshot.draftsByPath[chartPath] ?? currentSnapshot.documents[chartPath];
  if (!current) return null;

  const next: ChartDocument = {
    ...cloneValue(current),
    ...cloneValue(patch),
    encodings: {
      ...cloneValue(current.encodings),
      ...(patch.encodings ? cloneValue(patch.encodings) : {}),
    },
  };
  const state = mutableSnapshot();
  state.draftsByPath[chartPath] = next;
  const committed = state.documents[chartPath];
  state.dirtyByPath[chartPath] =
    committed === undefined ||
    chartDocumentFingerprint(next) !== chartDocumentFingerprint(committed);
  publish(state);
  return currentSnapshot.draftsByPath[chartPath] ?? null;
}

export function discardChartDraft(chartPath: string): void {
  const state = mutableSnapshot();
  delete state.draftsByPath[chartPath];
  state.dirtyByPath[chartPath] = false;
  publish(state);
}

export function rebaseChartDraft(
  chartPath: string,
  _committed: DeepReadonly<ChartDocument>,
  expectedDraftFingerprint: string,
): "rebased" | "draft-changed" {
  const currentDraft = currentSnapshot.draftsByPath[chartPath];
  if (!currentDraft || chartDocumentFingerprint(currentDraft) !== expectedDraftFingerprint) {
    return "draft-changed";
  }

  const state = mutableSnapshot();
  delete state.draftsByPath[chartPath];
  state.dirtyByPath[chartPath] = false;
  publish(state);
  return "rebased";
}

function updatePendingSaveStatus(
  key: OptimisticOperationKey,
  status: PendingChartSave["status"],
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

export const chartProjectionPublication: ChartProjectionPublication = {
  replaceSnapshot: (snapshot) => {
    const state = mutableSnapshot();
    state.index = cloneValue(snapshot.index) as ChartIndexEntry[];
    state.documents = cloneValue(snapshot.documents) as Record<string, ChartDocument>;
    publish(state);
  },

  applyCommittedDocument: (chartPath, document) => {
    const state = mutableSnapshot();
    state.documents[chartPath] = cloneValue(document) as ChartDocument;
    publish(state);
  },

  removeCommittedDocument: (chartPath) => {
    const state = mutableSnapshot();
    delete state.documents[chartPath];
    state.index = state.index.filter((entry) => entry.chartPath !== chartPath);
    publish(state);
  },

  clearForProject: (projectInstanceId) => {
    activeProjectInstanceId = projectInstanceId;
    publish(emptyMutableSnapshot());
  },
};

export const chartSavePublication: ChartSavePublication = {
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

  markPendingSaveAcknowledged: (key) => updatePendingSaveStatus(key, "acknowledged"),

  markPendingSaveUnknown: (key) => updatePendingSaveStatus(key, "unknown"),

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
        Object.entries(records).filter(
          ([, record]) => record.projectInstanceId !== projectInstanceId,
        ),
      );
      if (Object.keys(retained).length === 0) delete state.pendingSaveByPath[path];
      else state.pendingSaveByPath[path] = retained;
    }
    publish(state);
  },
};

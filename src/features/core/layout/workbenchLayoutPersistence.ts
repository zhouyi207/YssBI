import {
  loadWorkbenchLayoutMemento,
  saveWorkbenchLayoutMemento,
  type WorkbenchLayoutMemento,
} from './workbenchLayoutMemento';

export const WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS = 250;

export type WorkbenchLayoutPersistSlice = 'parts' | 'editorGrid';

let persistTimer: ReturnType<typeof setTimeout> | null = null;
const pendingWrites: Partial<Record<WorkbenchLayoutPersistSlice, () => void>> = {};

function clearPersistTimer(): void {
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = null;
}

function runPendingWrites(): void {
  clearPersistTimer();
  const partsWrite = pendingWrites.parts;
  const editorGridWrite = pendingWrites.editorGrid;
  delete pendingWrites.parts;
  delete pendingWrites.editorGrid;
  partsWrite?.();
  editorGridWrite?.();
}

function clearPendingWrites(): void {
  clearPersistTimer();
  delete pendingWrites.parts;
  delete pendingWrites.editorGrid;
}

/** Merge a partial memento patch into localStorage (preserves unspecified fields). */
export function mergeWorkbenchLayoutMemento(patch: Partial<WorkbenchLayoutMemento>): void {
  const current = loadWorkbenchLayoutMemento();
  saveWorkbenchLayoutMemento({
    parts: patch.parts ?? current?.parts ?? {},
    editorGrid: patch.editorGrid !== undefined ? patch.editorGrid : (current?.editorGrid ?? null),
    editorTabs: patch.editorTabs !== undefined ? patch.editorTabs : (current?.editorTabs ?? null),
  });
}

export function scheduleWorkbenchLayoutPersist(
  slice: WorkbenchLayoutPersistSlice,
  write: () => void,
  delayMs = WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS,
): void {
  pendingWrites[slice] = write;
  clearPersistTimer();
  persistTimer = setTimeout(runPendingWrites, delayMs);
}

export function flushWorkbenchLayoutPersist(
  slice: WorkbenchLayoutPersistSlice,
  write: () => void,
): void {
  pendingWrites[slice] = write;
  runPendingWrites();
}

export function saveFullWorkbenchLayoutMemento(memento: WorkbenchLayoutMemento): void {
  clearPendingWrites();
  saveWorkbenchLayoutMemento(memento);
}

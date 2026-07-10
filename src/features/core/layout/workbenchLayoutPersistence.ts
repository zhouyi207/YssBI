import {
  loadWorkbenchLayoutMemento,
  saveWorkbenchLayoutMemento,
  type WorkbenchLayoutMemento,
} from './workbenchLayoutMemento';

export const WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS = 250;

let persistTimer: ReturnType<typeof setTimeout> | null = null;

/** Merge a partial memento patch into localStorage (preserves unspecified fields). */
export function mergeWorkbenchLayoutMemento(patch: Partial<WorkbenchLayoutMemento>): void {
  const current = loadWorkbenchLayoutMemento();
  saveWorkbenchLayoutMemento({
    parts: patch.parts ?? current?.parts ?? {},
    editorGrid: patch.editorGrid !== undefined ? patch.editorGrid : (current?.editorGrid ?? null),
  });
}

export function scheduleWorkbenchLayoutPersist(
  write: () => void,
  delayMs = WORKBENCH_LAYOUT_PERSIST_DEBOUNCE_MS,
): void {
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    persistTimer = null;
    write();
  }, delayMs);
}

export function flushWorkbenchLayoutPersist(write: () => void): void {
  if (persistTimer) {
    clearTimeout(persistTimer);
    persistTimer = null;
  }
  write();
}

export function saveFullWorkbenchLayoutMemento(memento: WorkbenchLayoutMemento): void {
  flushWorkbenchLayoutPersist(() => saveWorkbenchLayoutMemento(memento));
}

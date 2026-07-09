import type { WorkbenchPartId } from './workbenchLayoutDefaults';

const SASH_DRAG_BODY_CLASS = 'layout-sash-dragging';

export const PART_RESIZE_COMMIT_EVENT = 'workbench-part-resize-commit';

export type PartResizeCommitDetail = {
  partId: WorkbenchPartId;
  pixelSize: number;
};

const DEBOUNCE_MS = 100;
const pending = new Map<WorkbenchPartId, number>();
let timer: ReturnType<typeof setTimeout> | null = null;

function flushPartResizeCommits(): void {
  timer = null;
  if (typeof document !== 'undefined' && document.body.classList.contains(SASH_DRAG_BODY_CLASS)) {
    return;
  }
  for (const [partId, pixelSize] of pending.entries()) {
    window.dispatchEvent(
      new CustomEvent<PartResizeCommitDetail>(PART_RESIZE_COMMIT_EVENT, {
        detail: { partId, pixelSize },
      }),
    );
  }
  pending.clear();
}

/** Debounced part size commit — skipped while sash drag preview is active. */
export function schedulePartResizeCommit(partId: WorkbenchPartId, pixelSize: number): void {
  pending.set(partId, pixelSize);
  if (timer) clearTimeout(timer);
  timer = setTimeout(flushPartResizeCommits, DEBOUNCE_MS);
}

export function subscribePartResizeCommit(
  listener: (detail: PartResizeCommitDetail) => void,
): () => void {
  const handler = (event: Event) => {
    const custom = event as CustomEvent<PartResizeCommitDetail>;
    if (custom.detail) listener(custom.detail);
  };
  window.addEventListener(PART_RESIZE_COMMIT_EVENT, handler);
  return () => window.removeEventListener(PART_RESIZE_COMMIT_EVENT, handler);
}

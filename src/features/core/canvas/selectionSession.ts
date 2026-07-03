import type { SelectionHitTarget } from './selectionHitTargets';
import { clearAllSelectionPreview, queryCanvasElement } from './selectionHitTargets';

export type ActiveSelectionSession = {
  active: true;
  groupId: string;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  preserveSelection: boolean;
};

export type SelectionSession = ActiveSelectionSession | { active: false };

const IDLE: SelectionSession = { active: false };

let session: SelectionSession = IDLE;
/** Shared across all canvas interaction instances (session is module-global). */
let hitTargets: SelectionHitTarget[] = [];
let previewIds: string[] = [];
const listeners = new Set<() => void>();

function publish(): void {
  listeners.forEach((listener) => listener());
}

export function getSelectionSession(): SelectionSession {
  return session;
}

export function getSelectionHitTargets(): readonly SelectionHitTarget[] {
  return hitTargets;
}

export function setSelectionHitTargets(targets: readonly SelectionHitTarget[]): void {
  hitTargets = [...targets];
}

export function getSelectionPreviewIds(): readonly string[] {
  return previewIds;
}

export function setSelectionPreviewIds(ids: readonly string[]): void {
  previewIds = [...ids];
}

export function subscribeSelectionSession(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function startSelectionSession(input: {
  groupId: string;
  startX: number;
  startY: number;
  preserveSelection: boolean;
}): void {
  previewIds = [];
  session = {
    active: true,
    groupId: input.groupId,
    startX: input.startX,
    startY: input.startY,
    currentX: input.startX,
    currentY: input.startY,
    preserveSelection: input.preserveSelection,
  };
  publish();
}

export function updateSelectionSession(currentX: number, currentY: number): void {
  if (!session.active) return;
  if (session.currentX === currentX && session.currentY === currentY) return;
  session = { ...session, currentX, currentY };
  publish();
}

export function endSelectionSession(): void {
  if (!session.active) return;
  session = IDLE;
  hitTargets = [];
  previewIds = [];
  publish();
}

/** Clear preview DOM (if any) and tear down the live selection session. */
export function abortSelectionSession(groupId?: string): void {
  if (groupId) {
    const canvasEl = queryCanvasElement(groupId);
    if (canvasEl) clearAllSelectionPreview(canvasEl);
  }
  endSelectionSession();
}

export function selectionScreenRect(active: ActiveSelectionSession): {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
} {
  return {
    x1: Math.min(active.startX, active.currentX),
    y1: Math.min(active.startY, active.currentY),
    x2: Math.max(active.startX, active.currentX),
    y2: Math.max(active.startY, active.currentY),
  };
}

export function selectionSessionMoved(active: ActiveSelectionSession, thresholdPx: number): boolean {
  const dx = Math.abs(active.currentX - active.startX);
  const dy = Math.abs(active.currentY - active.startY);
  return dx > thresholdPx || dy > thresholdPx;
}

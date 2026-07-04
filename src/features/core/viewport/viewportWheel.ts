import type { GraphPosition } from '@/shared/types/domain';
import { clamp } from '@/shared/utils';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import {
  getViewport,
  scheduleViewportCommit,
  scheduleViewportPersist,
  setViewportLive,
} from './viewportSession';
import { persistGraphViewport } from './persistGraphViewport';

const ZOOM_CLAMP = { min: 0.1, max: 5 } as const;
const IGNORE_WHEEL_SELECTORS = ['.menubar-container', '.sidebar-container', '.menu-container'];

export function shouldIgnoreWheelTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return IGNORE_WHEEL_SELECTORS.some((selector) => target.closest(selector));
}

export function isPointerInsideRect(
  clientX: number,
  clientY: number,
  rect: DOMRect,
): boolean {
  return clientX >= rect.left && clientX <= rect.right
    && clientY >= rect.top && clientY <= rect.bottom;
}

export function applyWheelToViewport(
  current: GraphPosition,
  e: WheelEvent,
  canvasRect: DOMRect,
): GraphPosition {
  if (e.ctrlKey || e.metaKey) {
    const mouseX = e.clientX - canvasRect.left;
    const mouseY = e.clientY - canvasRect.top;
    const factor = Math.pow(1.1, -e.deltaY / 100);
    const nextScale = clamp(current.scale * factor, ZOOM_CLAMP.min, ZOOM_CLAMP.max);
    const worldX = (mouseX - current.x) / current.scale;
    const worldY = (mouseY - current.y) / current.scale;
    return {
      scale: nextScale,
      x: mouseX - worldX * nextScale,
      y: mouseY - worldY * nextScale,
    };
  }

  const panX = e.shiftKey && e.deltaX === 0 ? e.deltaY : e.deltaX;
  const panY = e.shiftKey && e.deltaX === 0 ? 0 : e.deltaY;
  return {
    ...current,
    x: current.x - panX,
    y: current.y - panY,
  };
}

export function attachViewportWheel(
  canvasEl: HTMLElement,
  graphId: string,
): () => void {
  const timers: { commit?: number | null; persist?: number | null } = {};

  const onWheel = (e: WheelEvent) => {
    if (shouldIgnoreWheelTarget(e.target)) return;

    const rect = canvasEl.getBoundingClientRect();
    if (!isPointerInsideRect(e.clientX, e.clientY, rect)) return;

    e.preventDefault();

    const next = applyWheelToViewport(getViewport(graphId), e, rect);
    setViewportLive(graphId, next);
    scheduleViewportCommit(graphId, timers);
    scheduleViewportPersist(graphId, () => persistGraphViewport(graphId), timers);
  };

  const cleanup = addGlobalEventListener(window, 'wheel', onWheel, { passive: false, capture: true });

  return () => {
    cleanup();
    if (timers.commit != null) window.clearTimeout(timers.commit);
    if (timers.persist != null) window.clearTimeout(timers.persist);
  };
}

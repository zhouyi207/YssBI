import type { EditorViewport } from './editorViewport';
import { clamp } from '@/shared/utils';
import {
  getViewport,
  scheduleViewportCommit,
  scheduleViewportPersist,
  setViewportLive,
} from './viewportSession';
import { persistGraphViewport } from './persistGraphViewport';
import type { ViewportScope } from './viewportScope';

const ZOOM_CLAMP = { min: 0.1, max: 5 } as const;



export function applyWheelZoomToViewport(
  current: EditorViewport,
  e: WheelEvent,
  canvasRect: DOMRect,
): EditorViewport {


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

/** Wheel zoom bound to the canvas element (not global window). */
export function attachCanvasWheelZoom(
  canvasEl: HTMLElement,
  scope: ViewportScope,
): () => void {
  const timers: { commit?: number | null; persist?: number | null } = {};

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const rect = canvasEl.getBoundingClientRect();
    const prev = getViewport(scope);
    const next = applyWheelZoomToViewport(prev, e, rect);
    if (prev.scale === next.scale && prev.x === next.x && prev.y === next.y) return;

    setViewportLive(scope, next);
    scheduleViewportCommit(scope, timers);
    scheduleViewportPersist(scope, () => persistGraphViewport(scope), timers);
  };

  canvasEl.addEventListener('wheel', onWheel, { passive: false });

  return () => {
    canvasEl.removeEventListener('wheel', onWheel);
    if (timers.commit != null) window.clearTimeout(timers.commit);
    if (timers.persist != null) window.clearTimeout(timers.persist);
  };
}

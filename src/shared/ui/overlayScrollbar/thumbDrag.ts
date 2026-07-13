import {
  scrollOffsetFromThumbPosition,
  thumbOffsetFromPointerDrag,
  thumbTravel,
  type ScrollbarAxisMetrics,
} from './metrics';

export type ScrollbarAxis = 'x' | 'y';

export interface ScrollbarThumbDragSession {
  axis: ScrollbarAxis;
  pointerId: number;
  originPointer: number;
  originThumbOffset: number;
  metrics: ScrollbarAxisMetrics;
}

export function pointerCoord(axis: ScrollbarAxis, event: { clientX: number; clientY: number }): number {
  return axis === 'y' ? event.clientY : event.clientX;
}

export function beginThumbDragSession(
  axis: ScrollbarAxis,
  event: PointerEvent,
  metrics: ScrollbarAxisMetrics,
): ScrollbarThumbDragSession | null {
  if (event.button !== 0) return null;
  return {
    axis,
    pointerId: event.pointerId,
    originPointer: pointerCoord(axis, event),
    originThumbOffset: metrics.thumbOffset,
    metrics,
  };
}

export function resolveThumbDragOffset(
  session: ScrollbarThumbDragSession,
  event: PointerEvent,
): number {
  return thumbOffsetFromPointerDrag(
    session.originThumbOffset,
    session.originPointer,
    pointerCoord(session.axis, event),
    thumbTravel(session.metrics),
  );
}

export function applyThumbDragPosition(options: {
  axis: ScrollbarAxis;
  thumbEl: HTMLElement;
  viewport: HTMLElement;
  thumbOffset: number;
  metrics: ScrollbarAxisMetrics;
}): void {
  const { axis, thumbEl, viewport, thumbOffset, metrics } = options;
  const scroll = scrollOffsetFromThumbPosition(thumbOffset, metrics);

  if (axis === 'y') {
    thumbEl.style.top = `${thumbOffset}px`;
    viewport.scrollTop = scroll;
    return;
  }

  thumbEl.style.left = `${thumbOffset}px`;
  viewport.scrollLeft = scroll;
}

export function withInstantViewportScroll(viewport: HTMLElement, action: () => void): void {
  const previous = viewport.style.scrollBehavior;
  viewport.style.scrollBehavior = 'auto';
  try {
    action();
  } finally {
    viewport.style.scrollBehavior = previous;
  }
}

/** Pointer listeners scoped to the scrollbar track element (no document/window). */
export function bindScrollbarThumbDrag(options: {
  captureHost: HTMLElement;
  thumbEl: HTMLElement;
  viewport: HTMLElement;
  session: ScrollbarThumbDragSession;
  onDraggingChange?: (dragging: boolean) => void;
  onDragEnd?: () => void;
}): () => void {
  const { captureHost, thumbEl, viewport, session, onDraggingChange, onDragEnd } = options;
  const { pointerId, axis, metrics } = session;

  captureHost.setPointerCapture(pointerId);
  onDraggingChange?.(true);
  viewport.style.scrollBehavior = 'auto';

  let dragged = false;
  let suppressNextClick = false;

  const onPointerMove = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    dragged = true;
    const thumbOffset = resolveThumbDragOffset(session, event);
    applyThumbDragPosition({ axis, thumbEl, viewport, thumbOffset, metrics });
  };

  const end = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    captureHost.releasePointerCapture(pointerId);
    captureHost.removeEventListener('pointermove', onPointerMove);
    captureHost.removeEventListener('pointerup', end);
    captureHost.removeEventListener('pointercancel', end);
    captureHost.removeEventListener('click', onClick, true);
    viewport.style.removeProperty('scroll-behavior');
    onDraggingChange?.(false);
    onDragEnd?.();
    if (dragged) suppressNextClick = true;
  };

  const onClick = (event: MouseEvent) => {
    if (!suppressNextClick) return;
    event.preventDefault();
    event.stopPropagation();
    suppressNextClick = false;
  };

  captureHost.addEventListener('pointermove', onPointerMove);
  captureHost.addEventListener('pointerup', end);
  captureHost.addEventListener('pointercancel', end);
  captureHost.addEventListener('click', onClick, true);

  return () => {
    captureHost.removeEventListener('pointermove', onPointerMove);
    captureHost.removeEventListener('pointerup', end);
    captureHost.removeEventListener('pointercancel', end);
    captureHost.removeEventListener('click', onClick, true);
    if (captureHost.hasPointerCapture(pointerId)) {
      captureHost.releasePointerCapture(pointerId);
    }
    viewport.style.removeProperty('scroll-behavior');
    onDraggingChange?.(false);
  };
}

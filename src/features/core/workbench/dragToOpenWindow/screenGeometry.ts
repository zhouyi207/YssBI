import type { AuxiliaryWindowBounds, DisplayBounds, ScreenPoint } from './types';

const DEFAULT_TITLE_BAR_HEIGHT = 30;

export function resolveCursorScreenPoint(
  event: Pick<DragEvent, 'screenX' | 'screenY'>,
  fallback?: ScreenPoint | null,
): ScreenPoint {
  const x = event.screenX ?? 0;
  const y = event.screenY ?? 0;
  if (x !== 0 || y !== 0) return { x, y };
  return fallback ?? { x: 0, y: 0 };
}

/** VS Code `maybeCreateAuxiliaryEditorPartAt` guard against accidental in-window release. */
export function isPointInsideFocusedWindow(
  point: ScreenPoint,
  targetWindow: Window = window,
): boolean {
  if (targetWindow.document.visibilityState !== 'visible' || !targetWindow.document.hasFocus()) {
    return false;
  }

  const left = targetWindow.screenX;
  const top = targetWindow.screenY;
  const right = left + targetWindow.outerWidth;
  const bottom = top + targetWindow.outerHeight;

  return point.x >= left && point.x <= right && point.y >= top && point.y <= bottom;
}

export function resolveAuxiliaryWindowBounds(
  cursorPoint: ScreenPoint,
  offsetElement: Pick<HTMLElement, 'offsetWidth' | 'offsetHeight'>,
  options?: {
    titleBarHeight?: number;
    display?: DisplayBounds | null;
  },
): AuxiliaryWindowBounds {
  const offsetX = offsetElement.offsetWidth / 2;
  const offsetY = (options?.titleBarHeight ?? DEFAULT_TITLE_BAR_HEIGHT) + offsetElement.offsetHeight / 2;

  let x = cursorPoint.x - offsetX;
  let y = cursorPoint.y - offsetY;

  const display = options?.display;
  if (display) {
    if (x < display.x) x = display.x;
    if (y < display.y) y = display.y;
  }

  return { x, y };
}

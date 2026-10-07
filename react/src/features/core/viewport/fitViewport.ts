import type { EditorViewport } from "./editorViewport";
import { EDITOR_VIEWPORT_SCALE_LIMITS } from "./editorViewport";

export interface WorldBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface FitWorldBoundsOptions {
  padding?: number;
  minScale?: number;
  maxScale?: number;
}

const DEFAULT_PADDING = 64;
const MIN_EXTENT = 1;

function positiveFinite(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

export function fitWorldBounds(
  bounds: WorldBounds,
  viewportSize: { width: number; height: number },
  options: FitWorldBoundsOptions = {},
): EditorViewport {
  const padding = Number.isFinite(options.padding)
    ? Math.max(0, options.padding ?? DEFAULT_PADDING)
    : DEFAULT_PADDING;
  const minScale = positiveFinite(
    options.minScale ?? EDITOR_VIEWPORT_SCALE_LIMITS.min,
    EDITOR_VIEWPORT_SCALE_LIMITS.min,
  );
  const maxScale = Math.max(
    minScale,
    positiveFinite(
      options.maxScale ?? EDITOR_VIEWPORT_SCALE_LIMITS.max,
      EDITOR_VIEWPORT_SCALE_LIMITS.max,
    ),
  );
  const viewportWidth = positiveFinite(viewportSize.width, MIN_EXTENT);
  const viewportHeight = positiveFinite(viewportSize.height, MIN_EXTENT);
  const boundsWidth = positiveFinite(Math.abs(bounds.right - bounds.left), MIN_EXTENT);
  const boundsHeight = positiveFinite(Math.abs(bounds.bottom - bounds.top), MIN_EXTENT);
  const availableWidth = Math.max(MIN_EXTENT, viewportWidth - padding * 2);
  const availableHeight = Math.max(MIN_EXTENT, viewportHeight - padding * 2);
  const unclampedScale = Math.min(availableWidth / boundsWidth, availableHeight / boundsHeight);
  const scale = Math.min(maxScale, Math.max(minScale, unclampedScale));
  const centerX = (bounds.left + bounds.right) / 2;
  const centerY = (bounds.top + bounds.bottom) / 2;

  return {
    x: viewportWidth / 2 - centerX * scale,
    y: viewportHeight / 2 - centerY * scale,
    scale,
  };
}

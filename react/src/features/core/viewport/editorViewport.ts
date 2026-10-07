import { DEFAULT_VIEWPORT } from "@/shared/config-default";

export const EDITOR_VIEWPORT_SCALE_LIMITS = { min: 0.1, max: 5 } as const;

/** Editor canvas pan/zoom — frontend-only; never persisted in graph files or sent to Rust. */
export interface EditorViewport {
  x: number;
  y: number;
  scale: number;
}

export function normalizeEditorViewport(viewport?: EditorViewport | null): EditorViewport {
  if (!viewport) return { ...DEFAULT_VIEWPORT };
  return {
    x: viewport.x ?? 0,
    y: viewport.y ?? 0,
    scale: viewport.scale ?? DEFAULT_VIEWPORT.scale,
  };
}

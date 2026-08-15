export const EDITOR_VIEWPORT_SCALE_LIMITS = { min: 0.1, max: 5 } as const;

/** Editor canvas pan/zoom — frontend-only; never persisted in graph files or sent to Rust. */
export interface EditorViewport {
  x: number;
  y: number;
  scale: number;
}

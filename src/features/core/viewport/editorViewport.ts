/** Editor canvas pan/zoom — frontend-only; never persisted in graph files or sent to Rust. */
export interface EditorViewport {
  x: number;
  y: number;
  scale: number;
}

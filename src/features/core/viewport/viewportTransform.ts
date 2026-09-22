import type { CSSProperties } from "react";
import type { EditorViewport } from "./editorViewport";

export function applyViewportGrid(
  el: HTMLElement,
  viewport: EditorViewport,
  gridSize: number,
): void {
  el.style.backgroundSize = `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`;
  el.style.backgroundPosition = `${viewport.x}px ${viewport.y}px`;
}

export function viewportGridStyle(viewport: EditorViewport, gridSize: number): CSSProperties {
  return {
    backgroundSize: `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`,
    backgroundPosition: `${viewport.x}px ${viewport.y}px`,
  };
}

import type { CSSProperties } from 'react';
import type { EditorViewport } from './editorViewport';

export function applyViewportTransform(el: HTMLElement, viewport: EditorViewport): void {
  el.style.transform = `translate3d(${viewport.x}px, ${viewport.y}px, 0) scale(${viewport.scale})`;
}

export function applyViewportGrid(el: HTMLElement, viewport: EditorViewport, gridSize: number): void {
  el.style.backgroundSize = `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`;
  el.style.backgroundPosition = `${viewport.x}px ${viewport.y}px`;
}

export function viewportTransformStyle(viewport: EditorViewport): CSSProperties {
  return {
    transform: `translate3d(${viewport.x}px, ${viewport.y}px, 0) scale(${viewport.scale})`,
    transformOrigin: '0 0',
    backfaceVisibility: 'hidden',
    WebkitBackfaceVisibility: 'hidden',
  };
}

export function viewportGridStyle(viewport: EditorViewport, gridSize: number): CSSProperties {
  return {
    backgroundSize: `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`,
    backgroundPosition: `${viewport.x}px ${viewport.y}px`,
  };
}

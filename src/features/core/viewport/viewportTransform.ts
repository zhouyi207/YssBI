import type { CSSProperties } from 'react';
import type { GraphPosition } from '@/shared/types/domain';

export function applyViewportTransform(el: HTMLElement, viewport: GraphPosition): void {
  el.style.transform = `translate3d(${viewport.x}px, ${viewport.y}px, 0) scale(${viewport.scale})`;
}

export function applyViewportGrid(el: HTMLElement, viewport: GraphPosition, gridSize: number): void {
  el.style.backgroundSize = `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`;
  el.style.backgroundPosition = `${viewport.x}px ${viewport.y}px`;
}

export function viewportTransformStyle(viewport: GraphPosition): CSSProperties {
  return {
    transform: `translate3d(${viewport.x}px, ${viewport.y}px, 0) scale(${viewport.scale})`,
    transformOrigin: '0 0',
    backfaceVisibility: 'hidden',
    WebkitBackfaceVisibility: 'hidden',
  };
}

export function viewportGridStyle(viewport: GraphPosition, gridSize: number): CSSProperties {
  return {
    backgroundSize: `${gridSize * viewport.scale}px ${gridSize * viewport.scale}px`,
    backgroundPosition: `${viewport.x}px ${viewport.y}px`,
  };
}

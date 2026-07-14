import type { DraggableSyntheticListeners } from '@dnd-kit/core';
import type { PointerEvent as ReactPointerEvent } from 'react';

/** Compose dnd-kit drag sensor with editor-group activation (VS Code tab mousedown). */
export function bindTabDragPointerDown(
  listeners: DraggableSyntheticListeners | undefined,
  onActivate: () => void,
  options?: { ignoreSelector?: string },
): (event: ReactPointerEvent) => void {
  const ignoreSelector = options?.ignoreSelector ?? 'button';
  return (event) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest(ignoreSelector)) {
      event.stopPropagation();
      return;
    }
    onActivate();
    listeners?.onPointerDown?.(event);
  };
}

/**
 * VS Code `onGroupDragStart`: only when `e.target === tabsContainer` (empty strip gap).
 */
export function bindEditorGroupStripPointerDown(
  listeners: DraggableSyntheticListeners | undefined,
  onActivate: () => void,
): (event: ReactPointerEvent) => void {
  return (event) => {
    if (event.button !== 0) return;
    if (event.target !== event.currentTarget) return;
    onActivate();
    listeners?.onPointerDown?.(event);
  };
}

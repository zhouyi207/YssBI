import { useCallback, useState } from 'react';
import type { MouseEvent } from 'react';
import type { ActionMenuPosition } from './ActionMenu';

export interface PositionedActionMenuState<TTarget> extends ActionMenuPosition {
  target: TTarget;
}

export function usePositionedActionMenu<TTarget>() {
  const [contextMenu, setContextMenu] = useState<PositionedActionMenuState<TTarget> | null>(null);

  const openActionMenu = useCallback((event: MouseEvent, target: TTarget) => {
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ x: event.clientX, y: event.clientY, target });
  }, []);

  const closeActionMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  return {
    contextMenu,
    setContextMenu,
    openActionMenu,
    closeActionMenu,
  };
}

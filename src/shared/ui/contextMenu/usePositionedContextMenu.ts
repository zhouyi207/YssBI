import { useCallback, useState } from "react";
import type { MouseEvent } from "react";

export interface PositionedContextMenuState<TTarget> {
  x: number;
  y: number;
  target: TTarget;
}

export function usePositionedContextMenu<TTarget>() {
  const [contextMenu, setContextMenu] = useState<PositionedContextMenuState<TTarget> | null>(null);

  const openContextMenu = useCallback((e: MouseEvent, target: TTarget) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, target });
  }, []);

  const closeContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  return {
    contextMenu,
    setContextMenu,
    openContextMenu,
    closeContextMenu,
  };
}

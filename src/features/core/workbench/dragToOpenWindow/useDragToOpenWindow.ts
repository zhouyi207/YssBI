import { useCallback, useEffect, useRef, type RefObject } from 'react';
import { applyWorkbenchDragImage } from './applyWorkbenchDragImage';
import { acceptsDragStart, type DragSurfaceMode } from './dragSurface';
import { isDragToOpenWindowOperation } from './dragToOpenWindowPolicy';
import { shouldOpenAuxiliaryWindowOnDragEnd } from './evaluateDragToOpenWindow';
import { fillWorkbenchDragTransfer } from './fillWorkbenchDragTransfer';
import { installGlobalWindowDraggedOverTracker } from './globalWindowDraggedOverTracker';
import { resolveAuxiliaryWindowBounds, resolveCursorScreenPoint } from './screenGeometry';
import type { AuxiliaryWindowBounds, ScreenPoint } from './types';
import type { WorkbenchDragPayload } from './workbenchDragTypes';
import { workbenchDragTransfer } from './workbenchDragTransfer';

export type DragToOpenWindowHandleProps = {
  draggable: true;
  onDragStart: (event: React.DragEvent<HTMLElement>) => void;
  onDrag: (event: React.DragEvent<HTMLElement>) => void;
  onDragEnd: (event: React.DragEvent<HTMLElement>) => void;
};

export type UseDragToOpenWindowOptions = {
  enabled: boolean;
  dragPayload: WorkbenchDragPayload;
  dragImageLabel: string;
  /** VS Code `workbench.editor.dragToOpenWindow` */
  dragToOpenWindowEnabled: boolean;
  dragSurface?: DragSurfaceMode;
  onOpenAuxiliaryWindow: (bounds: AuxiliaryWindowBounds) => void | Promise<void>;
};

export function useDragToOpenWindow({
  enabled,
  dragPayload,
  dragImageLabel,
  dragToOpenWindowEnabled,
  dragSurface = 'strict',
  onOpenAuxiliaryWindow,
}: UseDragToOpenWindowOptions): {
  dragHandleRef: RefObject<HTMLDivElement | null>;
  dragHandleProps: DragToOpenWindowHandleProps | null;
} {
  const dragHandleRef = useRef<HTMLDivElement | null>(null);
  const isNewWindowOperationRef = useRef(false);
  const lastCursorPointRef = useRef<ScreenPoint | null>(null);

  useEffect(() => {
    if (!enabled) return;
    return installGlobalWindowDraggedOverTracker();
  }, [enabled]);

  const handleDrag = useCallback((event: React.DragEvent<HTMLElement>) => {
    if (!workbenchDragTransfer.hasData()) return;
    lastCursorPointRef.current = { x: event.screenX, y: event.screenY };
  }, []);

  const handleDragStart = useCallback((event: React.DragEvent<HTMLElement>) => {
    if (!enabled) return;
    if (!acceptsDragStart(event.nativeEvent, dragSurface)) {
      event.preventDefault();
      return;
    }

    const isNewWindowOperation = isDragToOpenWindowOperation(event, dragToOpenWindowEnabled);
    isNewWindowOperationRef.current = isNewWindowOperation;
    lastCursorPointRef.current = { x: event.screenX, y: event.screenY };

    fillWorkbenchDragTransfer(event.nativeEvent, dragPayload, {
      disableStandardTransfer: isNewWindowOperation,
    });
    applyWorkbenchDragImage(event.nativeEvent, event.currentTarget, dragImageLabel);
  }, [dragImageLabel, dragPayload, dragSurface, dragToOpenWindowEnabled, enabled]);

  const handleDragEnd = useCallback(async (event: React.DragEvent<HTMLElement>) => {
    if (!enabled) return;
    const dragElement = dragHandleRef.current;
    workbenchDragTransfer.clearData();

    if (!dragElement) return;

    const cursorPoint = resolveCursorScreenPoint(event.nativeEvent, lastCursorPointRef.current);
    lastCursorPointRef.current = null;

    if (!shouldOpenAuxiliaryWindowOnDragEnd({
      event: event.nativeEvent,
      dragElement,
      isNewWindowOperation: isNewWindowOperationRef.current,
      cursorPoint,
    })) {
      return;
    }

    const bounds = resolveAuxiliaryWindowBounds(cursorPoint, dragElement);
    await onOpenAuxiliaryWindow(bounds);
  }, [enabled, onOpenAuxiliaryWindow]);

  const dragHandleProps = enabled
    ? {
        draggable: true as const,
        onDragStart: handleDragStart,
        onDrag: handleDrag,
        onDragEnd: handleDragEnd,
      }
    : null;

  return { dragHandleRef, dragHandleProps };
}

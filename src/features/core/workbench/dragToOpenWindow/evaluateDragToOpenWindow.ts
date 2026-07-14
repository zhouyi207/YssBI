import { isWindowDraggedOver } from './globalWindowDraggedOverTracker';
import { isPointInsideFocusedWindow } from './screenGeometry';
import type { ScreenPoint } from './types';

export type DragToOpenWindowEndContext = {
  event: Pick<DragEvent, 'target'>;
  dragElement: HTMLElement;
  isNewWindowOperation: boolean;
  cursorPoint: ScreenPoint;
  targetWindow?: Window;
};

/**
 * VS Code `onGroupDragEnd` guards before `maybeCreateAuxiliaryEditorPartAt`.
 */
export function shouldOpenAuxiliaryWindowOnDragEnd(ctx: DragToOpenWindowEndContext): boolean {
  if (ctx.event.target !== ctx.dragElement) return false;
  if (!ctx.isNewWindowOperation) return false;
  if (isWindowDraggedOver()) return false;
  if (isPointInsideFocusedWindow(ctx.cursorPoint, ctx.targetWindow)) return false;
  return true;
}

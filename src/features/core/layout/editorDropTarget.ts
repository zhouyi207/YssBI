import { CANVAS_DROP_ZONE_ID_PREFIX } from '@/features/core/dnd';
import { listEditorGroupTabIds } from './editorTabStore';
import { isEditorGroupNode } from './layoutEditorGroupNode';
import { useLayoutStore } from './layoutStore';

const TAB_BAR_DROP_SELECTOR = '[data-tabbar-drop]';
const TAB_STRIP_SELECTOR = '[data-tab-strip]';

function layoutNodeElement(groupId: string): HTMLElement | null {
  return document.getElementById(`layout-node-${groupId}`);
}

function editorContentElement(groupId: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-editor-content="${groupId}"]`);
}

function pointInRect(x: number, y: number, rect: DOMRect): boolean {
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

/**
 * VS Code `getOverlayOffsetHeight` — with tabs use content below tab bar; empty group uses full shell.
 */
export function readEditorGroupDropBounds(groupId: string): DOMRect | null {
  const hasTabs = listEditorGroupTabIds(groupId).length > 0;
  if (hasTabs) {
    return editorContentElement(groupId)?.getBoundingClientRect() ?? null;
  }
  return layoutNodeElement(groupId)?.getBoundingClientRect() ?? null;
}

export interface TabBarInsertPreviewContext {
  /** Existing tab being reordered; null for external insert (e.g. sidebar graph). */
  draggedTabId: string | null;
  /** Source editor group for tab reorder; null for external insert. */
  sourceGroupId: string | null;
}

export function findTabBarTargetFromPointer(
  pointerX: number,
  pointerY: number,
): { groupId: string; stripElement: HTMLElement } | null {
  const dropTargets = document.querySelectorAll<HTMLElement>(TAB_BAR_DROP_SELECTOR);
  for (const dropElement of dropTargets) {
    const rect = dropElement.getBoundingClientRect();
    if (!pointInRect(pointerX, pointerY, rect)) continue;

    const groupId = dropElement.dataset.tabbarDrop;
    if (!groupId) continue;
    const stripElement = dropElement.querySelector<HTMLElement>(TAB_STRIP_SELECTOR) ?? dropElement;
    return { groupId, stripElement };
  }
  return null;
}

export function findTabUnderPointer(
  pointerX: number,
  pointerY: number,
): { groupId: string; tabId: string } | null {
  for (const element of document.elementsFromPoint(pointerX, pointerY)) {
    if (!(element instanceof HTMLElement)) continue;
    const tabElement = element.closest<HTMLElement>('[data-tab-id][data-tab-group]');
    if (!tabElement) continue;
    const tabId = tabElement.dataset.tabId;
    const groupId = tabElement.dataset.tabGroup;
    if (!tabId || !groupId) continue;
    return { groupId, tabId };
  }
  return null;
}

/** Resolve editor group under pointer using VS Code drop-surface bounds. */
export function findEditorGroupAtPointer(pointerX: number, pointerY: number): string | null {
  const nodes = useLayoutStore.getState().nodes;
  const seen = new Set<string>();

  for (const element of document.elementsFromPoint(pointerX, pointerY)) {
    if (!(element instanceof HTMLElement)) continue;

    const contentGroupId = element.dataset.editorContent;
    if (contentGroupId && !seen.has(contentGroupId)) {
      seen.add(contentGroupId);
      const bounds = readEditorGroupDropBounds(contentGroupId);
      if (bounds && pointInRect(pointerX, pointerY, bounds)) {
        return contentGroupId;
      }
    }

    const layoutId = element.id.startsWith('layout-node-')
      ? element.id.slice('layout-node-'.length)
      : null;
    if (!layoutId || seen.has(layoutId)) continue;
    const node = nodes[layoutId];
    if (!isEditorGroupNode(node)) continue;
    seen.add(layoutId);

    const bounds = readEditorGroupDropBounds(layoutId);
    if (bounds && pointInRect(pointerX, pointerY, bounds)) {
      return layoutId;
    }
  }

  return null;
}

export function findCanvasDropGroupId(
  pointerX: number,
  pointerY: number,
  canvasDropGroupId: string | null | undefined,
): string | null {
  if (canvasDropGroupId) return canvasDropGroupId;

  for (const element of document.elementsFromPoint(pointerX, pointerY)) {
    if (!(element instanceof HTMLElement)) continue;
    if (!element.id.startsWith(CANVAS_DROP_ZONE_ID_PREFIX)) continue;
    return element.id.slice(CANVAS_DROP_ZONE_ID_PREFIX.length);
  }

  return findEditorGroupAtPointer(pointerX, pointerY);
}

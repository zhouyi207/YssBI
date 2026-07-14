import type { LayoutTree } from '@/shared/types/ui';
import {
  DETAIL_PART_ID,
  EDITOR_AREA_ID,
  PANEL_PART_ID,
} from './workbenchLayoutDefaults';
import { SIDEBAR_NODE_ID } from './layoutStore';
import { isDescendantOf } from './editorGridLayout';

export const WORKBENCH_CHROME_PART_ATTR = 'data-workbench-chrome-part';
export const WORKBENCH_EDITOR_SURFACE_ATTR = 'data-workbench-editor-surface';

export const WORKBENCH_CHROME_PART_IDS = [
  SIDEBAR_NODE_ID,
  PANEL_PART_ID,
  DETAIL_PART_ID,
] as const;

export type WorkbenchChromePartId = (typeof WORKBENCH_CHROME_PART_IDS)[number];

export function resolveWorkbenchDropSurfaceFlags(
  nodeId: string,
  nodes: LayoutTree,
): { chromePart?: WorkbenchChromePartId; editorSurface?: boolean } {
  if (
    nodeId === SIDEBAR_NODE_ID
    || nodeId === PANEL_PART_ID
    || nodeId === DETAIL_PART_ID
  ) {
    return { chromePart: nodeId };
  }

  if (nodeId === EDITOR_AREA_ID || isDescendantOf(nodes, nodeId, EDITOR_AREA_ID)) {
    return { editorSurface: true };
  }

  return {};
}

function readChromePartFromElement(element: Element): WorkbenchChromePartId | null {
  const host = element.closest(`[${WORKBENCH_CHROME_PART_ATTR}]`);
  if (!host || typeof (host as HTMLElement).getAttribute !== 'function') return null;

  const part = host.getAttribute(WORKBENCH_CHROME_PART_ATTR);
  if (part === SIDEBAR_NODE_ID || part === PANEL_PART_ID || part === DETAIL_PART_ID) {
    return part;
  }

  return null;
}

export function findWorkbenchChromePartAtPointer(
  clientX: number,
  clientY: number,
): WorkbenchChromePartId | null {
  for (const element of document.elementsFromPoint(clientX, clientY)) {
    const part = readChromePartFromElement(element);
    if (part) return part;
  }
  return null;
}

export function isPointerOverWorkbenchEditorSurface(
  clientX: number,
  clientY: number,
): boolean {
  for (const element of document.elementsFromPoint(clientX, clientY)) {
    if (typeof (element as HTMLElement).closest === 'function'
      && element.closest(`[${WORKBENCH_EDITOR_SURFACE_ATTR}]`)) {
      return true;
    }
  }
  return false;
}

/** Sidebar palette items may only drop on the editor workbench — not chrome panels (sidebar/detail/logs). */
export function isSidebarItemDropAllowedAtPointer(
  clientX: number,
  clientY: number,
): boolean {
  if (findWorkbenchChromePartAtPointer(clientX, clientY)) return false;
  return isPointerOverWorkbenchEditorSurface(clientX, clientY);
}

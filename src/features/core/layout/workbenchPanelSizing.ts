import type { LayoutNode } from '@/shared/types/ui';
import { PANEL_PART_ID } from './workbenchLayoutDefaults';
import type { PanelPosition } from './panelPartLayout';
import { isPanelPositionHorizontal } from './panelPartLayout';

export const PANEL_MAX_VIEWPORT_RATIO = 0.8;

export interface WorkbenchViewport {
  width: number;
  height: number;
}

export function resolveWorkbenchViewport(): WorkbenchViewport {
  if (typeof window === 'undefined') {
    return { width: 1280, height: 720 };
  }
  return { width: window.innerWidth, height: window.innerHeight };
}

export function resolveWorkbenchPartMaxSize(
  node: LayoutNode,
  viewport: WorkbenchViewport = resolveWorkbenchViewport(),
  panelPosition: PanelPosition = 'bottom',
): number {
  const staticMax = node.maxSize ?? Number.POSITIVE_INFINITY;
  if (node.id === PANEL_PART_ID) {
    const axisSize = isPanelPositionHorizontal(panelPosition) ? viewport.width : viewport.height;
    return Math.min(staticMax, Math.floor(axisSize * PANEL_MAX_VIEWPORT_RATIO));
  }
  return staticMax;
}

export function clampWorkbenchPartSize(
  node: LayoutNode,
  size: number,
  viewport: WorkbenchViewport = resolveWorkbenchViewport(),
  panelPosition: PanelPosition = 'bottom',
): number {
  const min = node.minSize ?? 0;
  const max = resolveWorkbenchPartMaxSize(node, viewport, panelPosition);
  return Math.min(max, Math.max(min, size));
}

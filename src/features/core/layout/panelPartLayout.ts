import type { LayoutDirection, LayoutTree } from '@/shared/types/ui';
import { EDITOR_AREA_ID, PANEL_PART_ID } from './workbenchLayoutDefaults';

export const PANEL_POSITION_SETTINGS = ['Bottom', 'Left', 'Right'] as const;
export type PanelPositionSetting = (typeof PANEL_POSITION_SETTINGS)[number];

export type PanelPosition = 'bottom' | 'left' | 'right';

export const CENTER_NODE_ID = 'center';

export function normalizePanelPosition(value: string | undefined): PanelPosition {
  switch (value?.toLowerCase()) {
    case 'left':
      return 'left';
    case 'right':
      return 'right';
    default:
      return 'bottom';
  }
}

export function panelPositionToSetting(position: PanelPosition): PanelPositionSetting {
  if (position === 'left') return 'Left';
  if (position === 'right') return 'Right';
  return 'Bottom';
}

/** Derive panel dock side from the workbench `center` container. */
export function inferPanelPosition(nodes: LayoutTree): PanelPosition {
  const center = nodes[CENTER_NODE_ID];
  if (!center?.children?.length) return 'bottom';
  if (center.type === 'col') return 'bottom';

  const panelIdx = center.children.indexOf(PANEL_PART_ID);
  const editorIdx = center.children.indexOf(EDITOR_AREA_ID);
  if (panelIdx === -1 || editorIdx === -1) return 'bottom';
  return panelIdx < editorIdx ? 'left' : 'right';
}

export function centerLayoutForPanelPosition(position: PanelPosition): {
  type: LayoutDirection;
  children: string[];
} {
  switch (position) {
    case 'left':
      return { type: 'row', children: [PANEL_PART_ID, EDITOR_AREA_ID] };
    case 'right':
      return { type: 'row', children: [EDITOR_AREA_ID, PANEL_PART_ID] };
    default:
      return { type: 'col', children: [EDITOR_AREA_ID, PANEL_PART_ID] };
  }
}

export function isPanelPositionHorizontal(position: PanelPosition): boolean {
  return position === 'left' || position === 'right';
}

/** Editor ↔ Panel sash (any dock side). */
export function isEditorPanelSash(beforeNodeId: string, afterNodeId: string): boolean {
  const ids = new Set([beforeNodeId, afterNodeId]);
  return ids.has(EDITOR_AREA_ID) && ids.has(PANEL_PART_ID);
}

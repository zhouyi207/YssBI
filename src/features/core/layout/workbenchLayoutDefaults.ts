import type { LayoutTree } from '@/shared/types/ui';
import { DEFAULT_PANEL_VIEWS } from './panelPartModel';

export const WORKBENCH_ROOT_ID = 'root';
export const EDITOR_AREA_ID = 'editor_area';
export const DEFAULT_EDITOR_GROUP_ID = 'default_editor';

export const WORKBENCH_PART_IDS = ['sidebar', 'panel', 'detail'] as const;
export type WorkbenchPartId = (typeof WORKBENCH_PART_IDS)[number];

export const PANEL_PART_ID = 'panel';
export const DETAIL_PART_ID = 'detail';

/** VS Code-style: sidebar | center(editor+panel) | detail */
export function createInitialWorkbenchNodes(): LayoutTree {
  return {
    [WORKBENCH_ROOT_ID]: {
      id: WORKBENCH_ROOT_ID,
      type: 'row',
      parentId: null,
      children: ['sidebar', 'center', 'detail'],
    },
    sidebar: {
      id: 'sidebar',
      type: 'component',
      parentId: WORKBENCH_ROOT_ID,
      pixelSize: 260,
      minSize: 240,
      data: { component: 'Sidebar', visible: true, title: 'Explorer', isFixed: true, currentTab: 'graphs', userHidden: false },
    },
    center: {
      id: 'center',
      type: 'col',
      parentId: WORKBENCH_ROOT_ID,
      children: [EDITOR_AREA_ID, 'panel'],
      size: 1,
    },
    [EDITOR_AREA_ID]: {
      id: EDITOR_AREA_ID,
      type: 'row',
      parentId: 'center',
      children: [DEFAULT_EDITOR_GROUP_ID],
      size: 1,
    },
    [DEFAULT_EDITOR_GROUP_ID]: {
      id: DEFAULT_EDITOR_GROUP_ID,
      type: 'component',
      parentId: EDITOR_AREA_ID,
      data: {
        component: 'GraphEditor',
      },
    },
    panel: {
      id: 'panel',
      type: 'component',
      parentId: 'center',
      pixelSize: 200,
      minSize: 80,
      data: {
        component: 'PanelPart',
        panelViews: DEFAULT_PANEL_VIEWS,
        activePanelView: 'logs',
        visible: true,
        title: 'Panel',
        isFixed: true,
        userHidden: false,
      },
    },
    detail: {
      id: 'detail',
      type: 'component',
      parentId: WORKBENCH_ROOT_ID,
      pixelSize: 300,
      minSize: 240,
      data: { component: 'Detail', visible: true, title: 'Properties', isFixed: true, userHidden: false },
    },
  };
}

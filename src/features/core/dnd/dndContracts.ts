export const DRAG_TYPES = {
  NODE_TEMPLATE: "node-template",
  GRAPH_RESOURCE: "graph-resource",
  TAB: "tab",
  LEAF: "leaf",
} as const;

export const DROP_TYPES = {
  CANVAS: "canvas",
  TABBAR: "tabbar",
  LAYOUT_REGION: "layout-region",
} as const;

export type GraphResourceDragData = {
  id: string;
  name: string;
  type: "event" | "function";
};

export type CanvasDropData = {
  dropType: typeof DROP_TYPES.CANVAS;
  groupId: string;
};

export type TabbarDropData = {
  dropType: typeof DROP_TYPES.TABBAR;
  targetNodeId: string;
  targetTabIndex: number;
};

export type LayoutRegionDropData = {
  dropType: typeof DROP_TYPES.LAYOUT_REGION;
  targetNodeId: string;
  dropPosition: "center" | "top" | "bottom" | "left" | "right";
};

export type KnownDropData =
  | CanvasDropData
  | TabbarDropData
  | LayoutRegionDropData;

export const CANVAS_DROP_ZONE_ID_PREFIX = "canvas-drop-zone-";

export function getCanvasDropZoneId(groupId: string) {
  return `${CANVAS_DROP_ZONE_ID_PREFIX}${groupId}`;
}

export function isCanvasDrop(data: unknown): data is CanvasDropData {
  return (data as { dropType?: unknown } | null)?.dropType === DROP_TYPES.CANVAS;
}

export function isTabbarDrop(data: unknown): data is TabbarDropData {
  return (data as { dropType?: unknown } | null)?.dropType === DROP_TYPES.TABBAR;
}

export function isLayoutRegionDrop(data: unknown): data is LayoutRegionDropData {
  return (data as { dropType?: unknown } | null)?.dropType === DROP_TYPES.LAYOUT_REGION;
}

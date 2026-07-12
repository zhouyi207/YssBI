export const DRAG_TYPES = {
  NODE_TEMPLATE: "node-template",
  GRAPH_RESOURCE: "graph-resource",
  TAB: "tab",
  LEAF: "leaf",
} as const;

export type DragType = (typeof DRAG_TYPES)[keyof typeof DRAG_TYPES];

export const DROP_TYPES = {
  CANVAS: "canvas",
  TABBAR: "tabbar",
  LAYOUT_REGION: "layout-region",
} as const;

/** Sidebar / palette 拖到画布时写入节点的模板字段 */
export type NodeSpawnTemplate = {
  title?: string;
  nodeType: string;
  category?: string;
  variableId?: string;
  variableName?: string;
  subGraphPath?: string;
};

export type GraphResourceDragData = {
  id: string;
  name: string;
  type: "event" | "function";
};

export type NodeTemplateDragData = {
  type: typeof DRAG_TYPES.NODE_TEMPLATE;
  template: NodeSpawnTemplate;
  sidebarResource?: GraphResourceDragData;
};

/** palette → canvas 的 node-template payload（与 `NodeTemplateDragData` 同义，端到端类型链别名） */
export type NodeTemplateDragPayload = NodeTemplateDragData;

export type GraphResourceDragPayload = {
  type: typeof DRAG_TYPES.GRAPH_RESOURCE;
  sidebarResource: GraphResourceDragData;
};

export type TabDragData = {
  type: typeof DRAG_TYPES.TAB;
  tabId: string;
  sourceNodeId: string;
};

export type LeafDragData = {
  type: typeof DRAG_TYPES.LEAF;
  node: { id: string };
};

/** dnd-kit `active.data.current` 已知 drag source 联合类型 */
export type CanvasDragPayload =
  | NodeTemplateDragData
  | GraphResourceDragPayload
  | TabDragData
  | LeafDragData;

/** Sidebar / palette 产生的可落画布 payload */
export type SidebarDragPayload = NodeTemplateDragData | GraphResourceDragPayload;

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasDragType(data: unknown, type: DragType): data is Record<string, unknown> & { type: DragType } {
  return isRecord(data) && data.type === type;
}

export function isNodeTemplateDragData(data: unknown): data is NodeTemplateDragData {
  if (!hasDragType(data, DRAG_TYPES.NODE_TEMPLATE)) return false;
  const template = data.template;
  return isRecord(template) && typeof template.nodeType === "string";
}

export function isGraphResourceDragPayload(data: unknown): data is GraphResourceDragPayload {
  if (!hasDragType(data, DRAG_TYPES.GRAPH_RESOURCE)) return false;
  const resource = data.sidebarResource;
  if (!isRecord(resource)) return false;
  if (typeof resource.id !== "string" || typeof resource.name !== "string") return false;
  return resource.type === "event" || resource.type === "function";
}

export function isTabDragData(data: unknown): data is TabDragData {
  if (!hasDragType(data, DRAG_TYPES.TAB)) return false;
  return typeof data.tabId === "string" && typeof data.sourceNodeId === "string";
}

export function isLeafDragData(data: unknown): data is LeafDragData {
  if (!hasDragType(data, DRAG_TYPES.LEAF)) return false;
  const node = data.node;
  return isRecord(node) && typeof node.id === "string";
}

export function parseCanvasDragPayload(data: unknown): CanvasDragPayload | null {
  if (isNodeTemplateDragData(data)) return data;
  if (isGraphResourceDragPayload(data)) return data;
  if (isTabDragData(data)) return data;
  if (isLeafDragData(data)) return data;
  return null;
}

export function isSidebarSpawnDrag(data: unknown): data is SidebarDragPayload {
  return isNodeTemplateDragData(data) || isGraphResourceDragPayload(data);
}

export function getSidebarResourceFromDrag(
  data: CanvasDragPayload | null | undefined,
): GraphResourceDragData | undefined {
  if (isGraphResourceDragPayload(data)) return data.sidebarResource;
  if (isNodeTemplateDragData(data)) return data.sidebarResource;
  return undefined;
}

/** Sidebar 拖拽进行中写入 store 的 node-template 态（落画布 spawn） */
export type NodeTemplateDragState = {
  type: typeof DRAG_TYPES.NODE_TEMPLATE;
  template: NodeSpawnTemplate;
  sidebarResource?: GraphResourceDragData;
  x: number;
  y: number;
};

/** Sidebar 拖拽进行中写入 store 的 graph-resource 态（落画布开 tab） */
export type GraphResourceDragState = {
  type: typeof DRAG_TYPES.GRAPH_RESOURCE;
  sidebarResource: GraphResourceDragData;
  x: number;
  y: number;
};

export type SidebarDragState = NodeTemplateDragState | GraphResourceDragState;

export function isNodeTemplateDragState(state: SidebarDragState): state is NodeTemplateDragState {
  return state.type === DRAG_TYPES.NODE_TEMPLATE;
}

export function getSidebarResourceFromDragState(
  state: SidebarDragState | null | undefined,
): GraphResourceDragData | undefined {
  if (!state) return undefined;
  if (state.type === DRAG_TYPES.GRAPH_RESOURCE) return state.sidebarResource;
  return state.sidebarResource;
}

export function getSidebarDragOverlayLabel(state: SidebarDragState): string {
  if (state.type === DRAG_TYPES.GRAPH_RESOURCE) return state.sidebarResource.name;
  return state.template.title ?? state.template.nodeType;
}

export function getSpawnDragTitle(data: SidebarDragPayload): string {
  if (isGraphResourceDragPayload(data)) return data.sidebarResource.name;
  return (
    data.sidebarResource?.name
    ?? data.template.title
    ?? data.template.variableName
    ?? data.template.nodeType
  );
}

export function buildSidebarDragState(
  payload: SidebarDragPayload,
  x: number,
  y: number,
): SidebarDragState {
  if (isGraphResourceDragPayload(payload)) {
    return {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: payload.sidebarResource,
      x,
      y,
    };
  }

  const title = getSpawnDragTitle(payload);
  return {
    type: DRAG_TYPES.NODE_TEMPLATE,
    template: { ...payload.template, title },
    sidebarResource: payload.sidebarResource,
    x,
    y,
  };
}

import {
  isNodeCreationDescriptor,
  type NodeCreationDescriptor,
} from "@/features/domain/nodeCatalog/creationDescriptor";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

export const DRAG_TYPES = {
  NODE_TEMPLATE: "node-template",
  GRAPH_RESOURCE: "graph-resource",
  GRAPH_CONSTANT: "graph-constant",
} as const;

export type DragType = (typeof DRAG_TYPES)[keyof typeof DRAG_TYPES];

export const DROP_TYPES = {
  CANVAS: "canvas",
} as const;

/** Backend-issued descriptor forwarded unchanged when a template is dropped. */
export type NodeSpawnTemplate = {
  title?: string;
  descriptor: NodeCreationDescriptor;
};

export type GraphResourceDragData = {
  id: string;
  name: string;
  type: "event_graph" | "function_graph";
};

export type NodeTemplateDragData = {
  type: typeof DRAG_TYPES.NODE_TEMPLATE;
  template: NodeSpawnTemplate;
  sidebarResource?: GraphResourceDragData;
};

export type GraphResourceDragPayload = {
  type: typeof DRAG_TYPES.GRAPH_RESOURCE;
  sidebarResource: GraphResourceDragData;
};

export type GraphConstantDragPayload = {
  type: typeof DRAG_TYPES.GRAPH_CONSTANT;
  graphPath: string;
  constantId: string;
  name: string;
};

/** dnd-kit `active.data.current` 已知 drag source 联合类型 */
export type CanvasDragPayload =
  | NodeTemplateDragData
  | GraphResourceDragPayload
  | GraphConstantDragPayload;

/** Sidebar / palette / Details 产生的可落画布 payload */
export type SidebarDragPayload = CanvasDragPayload;

export const CANVAS_DROP_ZONE_ID_PREFIX = "canvas-drop-zone-";

export function getCanvasDropZoneId(panelInstanceId: string) {
  return `${CANVAS_DROP_ZONE_ID_PREFIX}${panelInstanceId}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasDragType(
  data: unknown,
  type: DragType,
): data is Record<string, unknown> & { type: DragType } {
  return isRecord(data) && data.type === type;
}

export function isNodeTemplateDragData(data: unknown): data is NodeTemplateDragData {
  if (!hasDragType(data, DRAG_TYPES.NODE_TEMPLATE)) return false;
  const template = data.template;
  if (!isRecord(template)) return false;
  const keys = Object.keys(template);
  return (
    keys.every((key) => key === "title" || key === "descriptor") &&
    keys.includes("descriptor") &&
    (template.title === undefined || typeof template.title === "string") &&
    isNodeCreationDescriptor(template.descriptor)
  );
}

export function isGraphResourceDragPayload(data: unknown): data is GraphResourceDragPayload {
  if (!hasDragType(data, DRAG_TYPES.GRAPH_RESOURCE)) return false;
  const resource = data.sidebarResource;
  if (!isRecord(resource)) return false;
  if (typeof resource.id !== "string" || typeof resource.name !== "string") return false;
  return resource.type === "event_graph" || resource.type === "function_graph";
}

export function isGraphConstantDragPayload(data: unknown): data is GraphConstantDragPayload {
  return (
    hasDragType(data, DRAG_TYPES.GRAPH_CONSTANT) &&
    typeof data.graphPath === "string" &&
    data.graphPath.length > 0 &&
    typeof data.constantId === "string" &&
    data.constantId.length > 0 &&
    typeof data.name === "string"
  );
}

export function parseCanvasDragPayload(data: unknown): CanvasDragPayload | null {
  if (isNodeTemplateDragData(data)) return data;
  if (isGraphResourceDragPayload(data)) return data;
  if (isGraphConstantDragPayload(data)) return data;
  return null;
}

export function isSidebarSpawnDrag(data: unknown): data is SidebarDragPayload {
  return parseCanvasDragPayload(data) !== null;
}

export function isGraphResourceDragState(state: SidebarDragState): state is GraphResourceDragState {
  return state.type === DRAG_TYPES.GRAPH_RESOURCE;
}

/** Sidebar 拖拽进行中写入 store 的 node-template 态（落画布 spawn） */
export type NodeTemplateDragState = {
  type: typeof DRAG_TYPES.NODE_TEMPLATE;
  template: NodeSpawnTemplate;
  sidebarResource?: GraphResourceDragData;
  x: number;
  y: number;
};

/** Sidebar 拖拽进行中写入 store 的 graph-resource 态（函数创建 Call 节点，事件打开图） */
export type GraphResourceDragState = {
  type: typeof DRAG_TYPES.GRAPH_RESOURCE;
  sidebarResource: GraphResourceDragData;
  x: number;
  y: number;
};

export type GraphConstantDragState = GraphConstantDragPayload & { x: number; y: number };

export type SidebarDragState =
  | NodeTemplateDragState
  | GraphResourceDragState
  | GraphConstantDragState;

export function isNodeTemplateDragState(state: SidebarDragState): state is NodeTemplateDragState {
  return state.type === DRAG_TYPES.NODE_TEMPLATE;
}

export function isGraphConstantDragState(state: SidebarDragState): state is GraphConstantDragState {
  return state.type === DRAG_TYPES.GRAPH_CONSTANT;
}

export function getSidebarDragOverlayLabel(state: DeepReadonly<SidebarDragState>): string {
  if (state.type === DRAG_TYPES.GRAPH_RESOURCE) return state.sidebarResource.name;
  if (state.type === DRAG_TYPES.GRAPH_CONSTANT) return state.name;
  return state.template.title ?? state.template.descriptor.nodeTypeId;
}

export function getSpawnDragTitle(data: SidebarDragPayload): string {
  if (isGraphResourceDragPayload(data)) return data.sidebarResource.name;
  if (isGraphConstantDragPayload(data)) return data.name;
  return data.sidebarResource?.name ?? data.template.title ?? data.template.descriptor.nodeTypeId;
}

export function buildSidebarDragState(
  payload: SidebarDragPayload,
  x: number,
  y: number,
): SidebarDragState {
  if (isGraphConstantDragPayload(payload)) return { ...payload, x, y };
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

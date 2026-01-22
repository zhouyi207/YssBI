import { Pin } from "./nodes";

export type CanvasState = {
  x: number;
  y: number;
  scale: number;
};

export type Gesture =
  | {
    type: "pan";
    lastX: number;
    lastY: number;
    moved: boolean;
    groupId?: string;
  }
  | {
    type: "select";
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
    groupId?: string;
  }
  | {
    type: "connect";
    startPin: Pin;
    startX: number; // 屏幕坐标
    startY: number;
    currentX: number;
    currentY: number;
    isReconnect?: boolean;
    groupId?: string;
  }
  | {
    type: "drag";
    nodeId?: string;
    lastX: number;
    lastY: number;
    moved: boolean;
    groupId?: string;
  }
  | null;

export interface Tab {
  id: string;
  title: string;
  type: "event" | "function" | "macro" | "project" | "setting";
  isDirty?: boolean;
}

export interface EditorGroup {
  id: string;
  tabs: Tab[];
  activeTabId: string | null;
  canvas: CanvasState;
  selectedNodeIds: string[];
}


export interface PinDefinition {
  id: string;
  name: string;
  type: string;
}
export interface SubGraphData {
  id: string;
  name: string;
  type: "event" | "function" | "macro";
  nodes: any[];
  canvas: CanvasState;
  variables: Record<string, { name: string; type: string; value: any }>;
  inputs?: PinDefinition[];
  outputs?: PinDefinition[];
}
export interface ProjectData {
  version: string;
  globalVariables: Record<string, { name: string; type: string; value: any }>;
  events: Record<string, SubGraphData>;
  functions: Record<string, SubGraphData>;
  macros: Record<string, SubGraphData>;
  metadata: {
    exportTime: string;
    appVersion: string;
  };
}

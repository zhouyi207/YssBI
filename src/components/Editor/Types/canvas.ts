import { Pin } from "./nodes";
import { VariableDefinition } from "./variables";

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
  // canvas: CanvasState; // Removed: managed by useViewportStore
  selectedNodeIds: string[];
  width?: number; // Added for resize support
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
  /** 局部变量（函数/宏作用域） */
  variables: Record<string, VariableDefinition>;
  inputs?: PinDefinition[];
  outputs?: PinDefinition[];
}

export interface DataFrameColumn {
  name: string;
  type: string;
}

export interface DataFrameData {
  id: string;
  name: string;
  columns: DataFrameColumn[];
  rows: any[][];
  rowCount: number;
  columnCount: number;
  sourcePath?: string;
}

export interface ProjectData {
  /** 全局变量 */
  globalVariables: Record<string, VariableDefinition>;
  events: Record<string, SubGraphData>;
  functions: Record<string, SubGraphData>;
  macros: Record<string, SubGraphData>;
  /** 数据帧 */
  dataframes: Record<string, DataFrameData>;
  metadata: {
    exportTime: string;
    appVersion: string;
  };
}

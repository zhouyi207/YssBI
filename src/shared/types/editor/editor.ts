import { Variable } from "./variables";



export type EditorGesture =
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


export interface EditorGroup {
  id: string;
  tabs: EditorTab[];
  activeTabId: string | null;
  // canvas: CanvasState; // Removed: managed by useViewportStore
  selectedNodeIds: string[];
  width?: number; // Added for resize support
}



export interface EditorTab {
  id: string;
  title: string;
  type: "event" | "function" | "macro" | "project" | "setting";
  isDirty?: boolean;
}


/**
 * Connection represents a directed relationship between two pins
 * This is the single source of truth for all connection relationships
 */



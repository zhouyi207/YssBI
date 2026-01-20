import { Pin } from "../node/models";

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
  }
  | {
    type: "select";
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  }
  | {
    type: "connect";
    startPin: Pin;
    startX: number; // 屏幕坐标
    startY: number;
    currentX: number;
    currentY: number;
    isReconnect?: boolean;
  }
  | {
    type: "disconnect";
    pin: Pin;
  }
  | null;

export interface Tab {
  id: string;
  title: string;
  path: string | null;
  isDirty: boolean;
}

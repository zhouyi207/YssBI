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
  | null;

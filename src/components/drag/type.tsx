export type DragState = null | {
  type: "node-template";
  template: any; // 你后面可以换成 NodeTemplate 类型
  x: number; // 当前鼠标位置（viewport）
  y: number;
  startX: number;
  startY: number;
};

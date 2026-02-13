export type DragState = {
    type: "node-template";
    template: any;
    x: number;
    y: number;
    startX: number;
    startY: number;
} | null;

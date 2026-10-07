// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { collectCanvasNodeWorldBounds } from "./canvasNodeBounds";

function setRect(element: HTMLElement, rect: Partial<DOMRect>): void {
  element.getBoundingClientRect = () => ({
    x: rect.left ?? 0,
    y: rect.top ?? 0,
    left: rect.left ?? 0,
    top: rect.top ?? 0,
    right: rect.right ?? 0,
    bottom: rect.bottom ?? 0,
    width: rect.width ?? (rect.right ?? 0) - (rect.left ?? 0),
    height: rect.height ?? (rect.bottom ?? 0) - (rect.top ?? 0),
    toJSON: () => ({}),
  });
}

function appendNode(canvas: HTMLElement, id: string, rect: Partial<DOMRect>): HTMLElement {
  const node = document.createElement("div");
  node.dataset.nodeId = id;
  setRect(node, rect);
  canvas.appendChild(node);
  return node;
}

describe("collectCanvasNodeWorldBounds", () => {
  it("reverses the active canvas viewport transform and unions live node rectangles", () => {
    const canvas = document.createElement("div");
    setRect(canvas, { left: 100, top: 50, right: 900, bottom: 650 });
    appendNode(canvas, "a", { left: 140, top: 100, right: 240, bottom: 180 });
    appendNode(canvas, "b", { left: 300, top: 80, right: 380, bottom: 220 });

    expect(
      collectCanvasNodeWorldBounds({
        canvasElement: canvas,
        viewport: { x: 20, y: 30, scale: 2 },
      }),
    ).toEqual({ left: 10, top: 0, right: 130, bottom: 70 });
  });

  it("includes only requested IDs and returns null when none are present", () => {
    const canvas = document.createElement("div");
    setRect(canvas, { left: 10, top: 20 });
    appendNode(canvas, "a", { left: 20, top: 40, right: 70, bottom: 90 });
    appendNode(canvas, "b", { left: 100, top: 120, right: 150, bottom: 170 });

    expect(
      collectCanvasNodeWorldBounds({
        canvasElement: canvas,
        viewport: { x: 0, y: 0, scale: 1 },
        nodeIds: ["b"],
      }),
    ).toEqual({ left: 90, top: 100, right: 140, bottom: 150 });
    expect(
      collectCanvasNodeWorldBounds({
        canvasElement: canvas,
        viewport: { x: 0, y: 0, scale: 1 },
        nodeIds: ["missing"],
      }),
    ).toBeNull();
  });
});

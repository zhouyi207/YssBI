import { describe, expect, it } from "vitest";
import { applyWheelZoomToViewport } from "./canvasWheelZoom";

describe("canvasWheelZoom", () => {
  const base = { x: 100, y: 50, scale: 1 };
  const rect = { left: 0, top: 0, width: 800, height: 600 } as DOMRect;

  it("zooms toward cursor with plain wheel", () => {
    const e = {
      ctrlKey: false,
      metaKey: false,
      deltaY: -100,
      clientX: 400,
      clientY: 300,
    } as WheelEvent;

    const next = applyWheelZoomToViewport(base, e, rect);
    expect(next.scale).toBeGreaterThan(base.scale);
    expect(next.x).not.toBe(base.x);
    expect(next.y).not.toBe(base.y);
  });

  it("does not require or reject modifier keys", () => {
    const plainWheel = {
      ctrlKey: false,
      metaKey: false,
      deltaY: 100,
      clientX: 100,
      clientY: 100,
    } as WheelEvent;
    const modifiedWheel = { ...plainWheel, ctrlKey: true } as WheelEvent;

    expect(applyWheelZoomToViewport(base, plainWheel, rect)).toEqual(
      applyWheelZoomToViewport(base, modifiedWheel, rect),
    );
  });
});

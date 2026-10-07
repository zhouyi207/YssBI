// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { resolveDragClientPoint } from "./dragEventInput";

describe("resolveDragClientPoint", () => {
  it("combines an activator client point with drag delta", () => {
    const activator = new MouseEvent("pointerdown", { clientX: 12, clientY: 24 });

    expect(resolveDragClientPoint({ activatorEvent: activator, delta: { x: 8, y: -4 } })).toEqual({
      x: 20,
      y: 20,
    });
  });

  it("returns null for a non-pointer activator", () => {
    expect(resolveDragClientPoint({ activatorEvent: new KeyboardEvent("keydown") })).toBeNull();
  });
});

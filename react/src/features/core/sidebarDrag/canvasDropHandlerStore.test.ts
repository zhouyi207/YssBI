import { expect, it, vi } from "vitest";
import { canvasDropHandlerStore } from "./canvasDropHandlerStore";

it("limits cleanup to its registration when a panel reuses the same callback", () => {
  const handler = vi.fn(async () => true);
  const otherHandler = vi.fn(async () => true);
  const unregisterFirst = canvasDropHandlerStore.registerHandler("panel", handler);
  const unregisterCurrent = canvasDropHandlerStore.registerHandler("panel", handler);
  const unregisterOther = canvasDropHandlerStore.registerHandler("other", otherHandler);
  try {
    unregisterFirst();
    expect(canvasDropHandlerStore.getHandler("panel")).toBe(handler);
    unregisterCurrent();
    unregisterFirst();
    expect(canvasDropHandlerStore.getHandler("panel")).toBeNull();
    expect(canvasDropHandlerStore.getHandler("other")).toBe(otherHandler);
  } finally {
    unregisterFirst();
    unregisterCurrent();
    unregisterOther();
  }
});

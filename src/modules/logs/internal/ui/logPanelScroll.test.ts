import { describe, expect, it } from "vitest";
import { isLogViewportPinnedToBottom } from "./logPanelScroll";

describe("logPanelScroll", () => {
  it("detects whether the viewport follows the live tail", () => {
    expect(isLogViewportPinnedToBottom(920, 1000, 100, 80)).toBe(true);
    expect(isLogViewportPinnedToBottom(800, 1000, 100, 80)).toBe(false);
  });
});

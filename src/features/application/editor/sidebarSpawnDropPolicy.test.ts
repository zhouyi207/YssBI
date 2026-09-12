import { describe, expect, it } from "vitest";
import { isSidebarSpawnDropAllowed } from "./sidebarSpawnDropPolicy";

describe("sidebarSpawnDropPolicy", () => {
  it("rejects non-sidebar drag payloads", () => {
    expect(
      isSidebarSpawnDropAllowed({ type: "tab", tabId: "a", sourceNodeId: "g" }, { x: 1, y: 1 }),
    ).toBe(false);
  });
});

import { describe, expect, it, vi } from "vitest";
import type { PinData } from "./graphRuntimeTypes";
import { toInteractionPinData } from "./interactionPinData";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";

describe("toInteractionPinData", () => {
  it("strips component capabilities before Pin data enters interaction state", () => {
    const pin: PinData & { contextMenuActions: { selectNode: () => void } } = {
      ...makeProjectedPinData({
        id: "pin-a",
        nodeId: "node-a",
        name: "Value",
        direction: "output",
        address: { kind: "declared", nodeId: "node-a", portKey: "value" },
      }),
      contextMenuActions: { selectNode: vi.fn() },
    };

    const interactionPin = toInteractionPinData(pin);

    expect(interactionPin).not.toHaveProperty("contextMenuActions");
    expect(() => structuredClone(interactionPin)).not.toThrow();
  });
});

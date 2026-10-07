import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyGraphMutation } from "@/features/application/graphEditing/graphEditCoordinator";
import { setNodeParameters } from "./setNodeParameters";

vi.mock("@/features/application/graphEditing/graphEditCoordinator", () => ({
  applyGraphMutation: vi.fn(),
}));

describe("setNodeParameters", () => {
  beforeEach(() => vi.clearAllMocks());

  it("forwards one exact atomic parameter edit through the Graph draft coordinator", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphMutation).mockResolvedValue(outcome);
    const parameters = {
      predicate: {
        column: "count",
        operator: "greaterThan",
        value: { type: "integer", value: "9007199254740993" },
      },
    };

    await expect(
      setNodeParameters({
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-1",
        locale: "en-US",
        parameters,
      }),
    ).resolves.toBe(outcome);

    expect(applyGraphMutation).toHaveBeenCalledOnce();
    expect(applyGraphMutation).toHaveBeenCalledWith({
      graphPath: "events/Main.yssbi-event",
      locale: "en-US",
      mutation: { type: "setParameters", payload: { nodeId: "node-1", parameters } },
    });
  });

  it("forwards partial values and resets for authoritative merging", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphMutation).mockResolvedValue(outcome);

    await setNodeParameters({
      graphPath: "events/Main.yssbi-event",
      nodeId: "node-1",
      locale: "en-US",
      parameters: { tolerance: 1e-6, max_iterations: null },
    });

    const { mutation } = vi.mocked(applyGraphMutation).mock.calls[0][0];
    expect(mutation).toEqual({
      type: "setParameters",
      payload: {
        nodeId: "node-1",
        parameters: { tolerance: 1e-6, max_iterations: null },
      },
    });
  });
});

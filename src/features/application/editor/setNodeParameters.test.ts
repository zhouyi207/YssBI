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
      mutation: expect.any(Function),
    });
    const { mutation } = vi.mocked(applyGraphMutation).mock.calls[0][0];
    if (typeof mutation !== "function") throw new Error("Expected a queued mutation factory");
    expect(mutation({ nodes: {} } as never)).toEqual({
      type: "setParameters",
      payload: { nodeId: "node-1", parameters },
    });
  });

  it("preserves draft parameters outside the edited field", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphMutation).mockResolvedValue(outcome);

    await setNodeParameters({
      graphPath: "events/Main.yssbi-event",
      nodeId: "node-1",
      locale: "en-US",
      parameters: { tolerance: 1e-6 },
    });

    const { mutation } = vi.mocked(applyGraphMutation).mock.calls[0][0];
    if (typeof mutation !== "function") throw new Error("Expected a queued mutation factory");
    expect(
      mutation({
        nodes: {
          "node-1": {
            parameters: { constant: true, tolerance: 1e-7, max_iterations: 100 },
          },
        },
      } as never),
    ).toEqual({
      type: "setParameters",
      payload: {
        nodeId: "node-1",
        parameters: { constant: true, tolerance: 1e-6, max_iterations: 100 },
      },
    });
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyGraphDraftMutation } from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { setNodeParameters } from "./setNodeParameters";

vi.mock("@/features/application/graphDraft/graphDraftCoordinator", () => ({
  applyGraphDraftMutation: vi.fn(),
}));

describe("setNodeParameters", () => {
  beforeEach(() => vi.clearAllMocks());

  it("forwards one exact atomic parameter edit through the Graph draft coordinator", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphDraftMutation).mockResolvedValue(outcome);
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

    expect(applyGraphDraftMutation).toHaveBeenCalledOnce();
    expect(applyGraphDraftMutation).toHaveBeenCalledWith({
      graphPath: "events/Main.yssbi-event",
      locale: "en-US",
      mutation: {
        type: "setParameters",
        payload: { nodeId: "node-1", parameters },
      },
    });
  });

  it("removes a null override while preserving the complete atomic parameter map", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphDraftMutation).mockResolvedValue(outcome);
    vi.spyOn(useGraphProjectionStore, "getState").mockReturnValue({
      getGraphNode: () => ({
        parameterEditors: [
          { key: "constant", value: true },
          { key: "convergence_tolerance", value: 1e-7 },
          { key: "missing_value_policy", value: "Reject" },
        ],
      }),
    } as never);

    await setNodeParameters({
      graphPath: "events/Main.yssbi-event",
      nodeId: "node-1",
      locale: "en-US",
      parameters: { convergence_tolerance: null },
    });

    expect(applyGraphDraftMutation).toHaveBeenCalledWith(
      expect.objectContaining({
        mutation: {
          type: "setParameters",
          payload: {
            nodeId: "node-1",
            parameters: { constant: true, missing_value_policy: "Reject" },
          },
        },
      }),
    );
  });
});

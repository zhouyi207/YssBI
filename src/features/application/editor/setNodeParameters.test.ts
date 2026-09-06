import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyGraphDraftMutation } from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphDraftStore } from "@/features/core/graphDraft/graphDraftStore";
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

  it("preserves draft parameters outside the edited field", async () => {
    const outcome = { status: "applied" as const, result: {} as never, insertedNodeIds: [] };
    vi.mocked(applyGraphDraftMutation).mockResolvedValue(outcome);
    vi.spyOn(useGraphDraftStore, "getState").mockReturnValue({
      sessions: {
        "events/Main.yssbi-event": {
          document: {
            nodes: {
              "node-1": {
                parameters: { constant: true, tolerance: 1e-7, max_iterations: 100 },
              },
            },
          },
        },
      },
    } as never);

    await setNodeParameters({
      graphPath: "events/Main.yssbi-event",
      nodeId: "node-1",
      locale: "en-US",
      parameters: { tolerance: 1e-6 },
    });

    expect(applyGraphDraftMutation).toHaveBeenCalledWith(
      expect.objectContaining({
        mutation: {
          type: "setParameters",
          payload: {
            nodeId: "node-1",
            parameters: { constant: true, tolerance: 1e-6, max_iterations: 100 },
          },
        },
      }),
    );
  });
});

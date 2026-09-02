import { describe, expect, it, vi } from "vitest";
import { createProjectEventConsumer } from "./projectEventConsumer";

describe("project event consumer", () => {
  it("refreshes from Rust for a matching resource publication without an operation ledger", async () => {
    const publishResourceMutationCommitted = vi.fn();
    const consumer = createProjectEventConsumer({
      hydration: {
        loadCurrentProject: vi.fn(),
        refreshResourceIndex: vi.fn(),
        replaceProject: vi.fn(),
      },
      currentProjectInstanceId: () => "project-a",
      publishResourceMutationCommitted,
    });
    const result = {
      operationId: "00000000-0000-0000-0000-000000000001",
      projectInstanceId: "project-a",
      publicationRevision: 2,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: "complete" as const, expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: false },
    };

    await expect(
      consumer.acceptEvent({
        type: "ResourceMutationCommitted",
        payload: { result },
      }),
    ).resolves.toEqual({ status: "applied" });
    expect(publishResourceMutationCommitted).toHaveBeenCalledWith(result);
  });
});

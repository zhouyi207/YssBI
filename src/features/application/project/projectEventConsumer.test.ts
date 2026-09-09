import { describe, expect, it, vi } from "vitest";
import { createProjectEventConsumer } from "./projectEventConsumer";

describe("project event consumer", () => {
  it("forwards a matching resource receipt to the publication owner", async () => {
    const publishResourceMutationCommitted = vi.fn();
    const consumer = createProjectEventConsumer({
      refreshResourceIndex: vi.fn(),
      activateProject: vi.fn(),
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

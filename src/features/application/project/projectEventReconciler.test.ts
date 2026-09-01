import { describe, expect, it, vi } from "vitest";
import {
  createProjectEventReconciler,
  type OptimisticOperationKey,
} from "./projectEventReconciler";

const key: OptimisticOperationKey = {
  projectInstanceId: "project-a",
  resourceKey: "events/analysis.yssbi-event",
  operationId: "operation-a",
  fromRevision: 4,
};

describe("project event reconciler", () => {
  it("invalidates one exact overlay and requests authoritative recovery for an unknown outcome", async () => {
    const invalidate = vi.fn();
    const recover = vi.fn(async () => undefined);
    const reconciler = createProjectEventReconciler({
      hydration: {
        loadCurrentProject: vi.fn(async () => ({ status: "published" as const })),
        refreshResourceIndex: vi.fn(async () => ({ status: "published" as const })),
        replaceProject: vi.fn(),
        loadGraph: vi.fn(async () => ({ status: "published" as const })),
      },
      currentProjectInstanceId: () => "project-a",
      invalidateOptimisticOperation: invalidate,
      requestAuthoritativeSnapshot: recover,
    });

    reconciler.acknowledgeOperation(key);
    await expect(reconciler.markUnknownOutcome(key)).resolves.toEqual({
      status: "recoveryRequested",
    });

    expect(invalidate).toHaveBeenCalledOnce();
    expect(invalidate).toHaveBeenCalledWith(key);
    expect(recover).toHaveBeenCalledOnce();
  });
});

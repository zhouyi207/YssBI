import { beforeEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock("@/sdk", () => ({ request: mocks.request }));
import type { BayesModelDraftDTO } from "@/shared/types/bayes";
import { getBayesInferenceStatus, submitBayesInference } from "./bayesInferenceService";

beforeEach(() => vi.clearAllMocks());
it("preserves uncertain outcomes and concrete backend progress", async () => {
  const progress = { stage: "sampling", completed: 32, total: 64 };
  const error = { code: "plugin_outcome_unknown", details: null, incidentId: null };
  mocks.request.mockResolvedValue({ taskId: "task", state: "outcomeUnknown", progress, error });
  await expect(getBayesInferenceStatus("task")).resolves.toEqual({
    taskId: "task",
    status: "outcome_unknown",
    progress,
    error,
  });
  expect(mocks.request).toHaveBeenCalledWith("tasks.get", { taskId: "task" });
});
it("forwards one logical operation identity and the selected timeout unchanged", async () => {
  mocks.request.mockResolvedValue({ taskId: "task", state: "admitted", error: null });
  const input = { model: "fixture" } as unknown as BayesModelDraftDTO;
  const options = { operationId: "op-1000-same", timeoutMs: 90_000 };
  await submitBayesInference(input, options);
  await submitBayesInference(input, options);
  expect(mocks.request).toHaveBeenCalledTimes(2);
  for (const call of mocks.request.mock.calls)
    expect(call).toEqual([
      "tasks.start",
      {
        taskType: "bayes.inference",
        parameters: input,
        operationId: options.operationId,
        timeoutMs: 90_000,
      },
    ]);
});

import { beforeEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock("@/sdk", () => ({ request: mocks.request }));
import { invokeCommand } from "./index";

beforeEach(() => vi.clearAllMocks());
it("preserves uncertain outcomes and concrete backend progress", async () => {
  const progress = { stage: "sampling", completed: 32, total: 64 };
  const error = { code: "plugin_outcome_unknown", details: null, incidentId: null };
  mocks.request.mockResolvedValue({ taskId: "task", state: "outcomeUnknown", progress, error });
  await expect(invokeCommand("get_bayes_inference_status", { taskId: "task" })).resolves.toEqual({
    taskId: "task",
    status: "outcome_unknown",
    progress,
    error,
  });
});
it("forwards one logical operation identity and the selected timeout unchanged", async () => {
  mocks.request.mockResolvedValue({ taskId: "task", state: "admitted", error: null });
  const args = { input: { model: "fixture" }, operationId: "op-1000-same", timeoutMs: 90_000 };
  await invokeCommand("submit_bayes_inference", args);
  await invokeCommand("submit_bayes_inference", args);
  for (const call of mocks.request.mock.calls)
    expect(call).toEqual([
      "tasks.start",
      {
        taskType: "bayes.inference",
        parameters: args.input,
        operationId: args.operationId,
        timeoutMs: 90_000,
      },
    ]);
});

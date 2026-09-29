import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  request: vi.fn(),
}));

vi.mock("@/sdk", () => ({
  request: mocks.request,
}));

import { getBayesInferenceStatus, readBayesInferenceResult } from "./bayesInferenceService";
import { validateBayesModel } from "./bayesModelService";

const task = {
  taskId: "task-42",
  state: "failed",
  progress: null,
  error: {
    code: "julia_bayes_sampling_failed",
    details: null,
    incidentId: "incident-42",
  },
};

const result = {
  summaries: [],
  diagnostics: {
    chains: 2,
    drawsPerChain: 100,
    warmup: 50,
    divergences: 0,
    maxTreedepthHits: 0,
    warnings: [],
  },
  artifactManifest: { taskId: "task-42", artifacts: [] },
};

const report = {
  ok: false,
  errors: [{ code: "dataset_required", severity: "error", path: "dataset" }],
  warnings: [],
};

describe("Bayes services enforce the current wire", () => {
  beforeEach(() => vi.clearAllMocks());

  it("parses asynchronous task responses instead of trusting a generic invoke cast", async () => {
    mocks.request.mockResolvedValueOnce(task);
    await expect(getBayesInferenceStatus("task-42")).resolves.toEqual({
      taskId: task.taskId,
      status: "failed",
      progress: task.progress,
      error: task.error,
    });

    mocks.request.mockResolvedValueOnce({
      ...task,
      error: { ...task.error, message: "legacy backend prose" },
    });
    await expect(getBayesInferenceStatus("task-42")).rejects.toThrow(
      "Invalid Bayes inference task response",
    );
  });

  it("rejects legacy warning prose in inference results", async () => {
    mocks.request.mockResolvedValueOnce({
      ...result,
      diagnostics: {
        ...result.diagnostics,
        warnings: [
          {
            code: "rhat_too_high",
            metric: "rhat",
            value: 1.2,
            threshold: 1.01,
            parameter: "beta",
            message: "legacy backend prose",
          },
        ],
      },
    });

    await expect(readBayesInferenceResult("task-42")).rejects.toThrow(
      "Invalid Bayes inference result response",
    );
    expect(mocks.request).toHaveBeenCalledWith("tasks.result", { taskId: "task-42" });
  });

  it("rejects legacy validation prose", async () => {
    mocks.request.mockResolvedValueOnce({
      ...report,
      errors: [{ ...report.errors[0], hint: "legacy backend prose" }],
    });

    await expect(validateBayesModel({} as never)).rejects.toThrow(
      "Invalid Bayes validation response",
    );
    expect(mocks.request).toHaveBeenCalledWith("commands.execute", {
      commandId: "validate_bayes_model",
      args: { input: {} },
    });
  });
});

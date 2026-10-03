import { describe, expect, it } from "vitest";
import type { BayesInferenceTaskDTO, InferenceResultDTO } from "@/shared/types/bayes";
import { bayesInferenceReducer, initialBayesInferenceState } from "./useBayesInferenceTask";

const task = (
  taskId: string,
  status: BayesInferenceTaskDTO["status"],
  error: BayesInferenceTaskDTO["error"] = null,
): BayesInferenceTaskDTO => ({ taskId, status, progress: null, error });
const result = (taskId: string): InferenceResultDTO => ({
  summaries: [],
  diagnostics: {
    chains: 2,
    drawsPerChain: 10,
    warmup: 5,
    divergences: null,
    maxTreedepthHits: null,
    warnings: [],
  },
  artifactManifest: { taskId, artifacts: [] },
});

describe("bayes inference task state machine", () => {
  it("ignores a submit response from an older request generation", () => {
    const first = bayesInferenceReducer(initialBayesInferenceState, {
      type: "submit_started",
      requestGeneration: 1,
    });
    const second = bayesInferenceReducer(first, { type: "submit_started", requestGeneration: 2 });
    const stale = bayesInferenceReducer(second, {
      type: "task_received",
      requestGeneration: 1,
      task: task("old", "running"),
    });
    expect(stale).toBe(second);
  });

  it("ignores status and result responses for another task", () => {
    const submitting = bayesInferenceReducer(initialBayesInferenceState, {
      type: "submit_started",
      requestGeneration: 1,
    });
    const active = bayesInferenceReducer(submitting, {
      type: "task_received",
      requestGeneration: 1,
      task: task("current", "running"),
    });
    expect(
      bayesInferenceReducer(active, {
        type: "task_received",
        requestGeneration: 1,
        task: task("old", "completed"),
      }),
    ).toBe(active);
    expect(
      bayesInferenceReducer(active, {
        type: "result_received",
        requestGeneration: 1,
        taskId: "old",
        result: result("old"),
      }),
    ).toBe(active);
  });

  it("preserves cancellation intent and terminal results against late polls and cancel failures", () => {
    const submitting = bayesInferenceReducer(initialBayesInferenceState, {
      type: "submit_started",
      requestGeneration: 1,
    });
    const active = bayesInferenceReducer(submitting, {
      type: "task_received",
      requestGeneration: 1,
      task: task("task-1", "running"),
    });
    const cancelling = bayesInferenceReducer(active, {
      type: "cancel_started",
      requestGeneration: 1,
      taskId: "task-1",
    });
    for (const status of ["running", "queued"] as const) {
      expect
        .soft(
          bayesInferenceReducer(cancelling, {
            type: "task_received",
            requestGeneration: 1,
            task: task("task-1", status),
          }).task?.status,
        )
        .toBe("cancelling");
    }
    expect(
      bayesInferenceReducer(cancelling, {
        type: "task_received",
        requestGeneration: 1,
        task: task("task-1", "cancelled"),
      }).phase,
    ).toBe("cancelled");
    const cancelFailure = {
      type: "cancel_failed" as const,
      requestGeneration: 1,
      taskId: "task-1",
      error: { code: "plugin_request_timeout", details: null, incidentId: null },
    };
    const uncertain = bayesInferenceReducer(cancelling, cancelFailure);
    expect(uncertain.phase).toBe("submission_unknown");
    const retrying = bayesInferenceReducer(uncertain, {
      type: "submit_started",
      requestGeneration: 2,
    });
    expect(
      bayesInferenceReducer(retrying, {
        type: "task_received",
        requestGeneration: 2,
        task: task("task-1", "running"),
      }).task?.status,
    ).toBe("running");
    const reading = bayesInferenceReducer(cancelling, {
      type: "task_received",
      requestGeneration: 1,
      task: task("task-1", "completed"),
    });
    expect(
      bayesInferenceReducer(reading, {
        type: "task_received",
        requestGeneration: 1,
        task: task("task-1", "running"),
      }),
    ).toBe(reading);
    expect.soft(bayesInferenceReducer(reading, cancelFailure)).toBe(reading);
    const completed = bayesInferenceReducer(reading, {
      type: "result_received",
      requestGeneration: 1,
      taskId: "task-1",
      result: result("task-1"),
    });
    expect.soft(bayesInferenceReducer(completed, cancelFailure)).toBe(completed);
    const resultFailure = bayesInferenceReducer(reading, {
      type: "request_failed",
      requestGeneration: 1,
      taskId: "task-1",
      error: { code: "plugin_result_missing", details: null, incidentId: null },
      uncertain: true,
    });
    expect(resultFailure).toMatchObject({
      phase: "submission_unknown",
      error: { code: "plugin_result_missing" },
    });
  });

  it("normalizes backend task failures into the same failed state as invoke errors", () => {
    const submitting = bayesInferenceReducer(initialBayesInferenceState, {
      type: "submit_started",
      requestGeneration: 1,
    });
    const failedTask = bayesInferenceReducer(submitting, {
      type: "task_received",
      requestGeneration: 1,
      task: task("task-1", "failed", {
        code: "julia_bayes_sampling_failed",
        details: null,
        incidentId: "incident-task-42",
      }),
    });
    expect(failedTask).toMatchObject({
      phase: "failed",
      error: {
        code: "julia_bayes_sampling_failed",
        details: null,
        incidentId: "incident-task-42",
      },
    });

    const invokeFailure = bayesInferenceReducer(submitting, {
      type: "request_failed",
      requestGeneration: 1,
      error: { code: "bayes_request_failed", details: null, incidentId: null },
    });
    expect(invokeFailure).toMatchObject({
      phase: "failed",
      error: { code: "bayes_request_failed", details: null, incidentId: null },
    });
  });

  it("rejects a result whose manifest belongs to another task", () => {
    const submitting = bayesInferenceReducer(initialBayesInferenceState, {
      type: "submit_started",
      requestGeneration: 1,
    });
    const reading = bayesInferenceReducer(submitting, {
      type: "task_received",
      requestGeneration: 1,
      task: task("current", "completed"),
    });

    expect(
      bayesInferenceReducer(reading, {
        type: "result_received",
        requestGeneration: 1,
        taskId: "current",
        result: result("old"),
      }),
    ).toBe(reading);
  });
});

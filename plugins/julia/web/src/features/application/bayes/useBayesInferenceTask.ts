import { createOperationId } from "@/sdk";
import { useEffect, useReducer, useRef } from "react";
import type {
  BayesInferenceTaskDTO,
  BayesModelDraftDTO,
  InferenceResultDTO,
} from "@/shared/types/bayes";
import {
  cancelBayesInference,
  getBayesInferenceStatus,
  readBayesInferenceResult,
  submitBayesInference,
} from "@/services/bayes";
import { normalizeBayesApplicationError, type BayesApplicationError } from "./bayesError";

const ACTIVE_TASK_STATUSES = new Set<BayesInferenceTaskDTO["status"]>([
  "queued",
  "running",
  "cancelling",
]);

export type BayesInferenceError = BayesApplicationError;

export interface BayesInferenceState {
  requestGeneration: number;
  phase:
    | "idle"
    | "submitting"
    | "active"
    | "reading_result"
    | "completed"
    | "cancelled"
    | "failed"
    | "outcome_unknown"
    | "submission_unknown";
  task: BayesInferenceTaskDTO | null;
  result: InferenceResultDTO | null;
  error: BayesInferenceError | null;
}

type BayesInferenceAction =
  | { type: "submit_started"; requestGeneration: number }
  | { type: "task_received"; requestGeneration: number; task: BayesInferenceTaskDTO }
  | {
      type: "result_received";
      requestGeneration: number;
      taskId: string;
      result: InferenceResultDTO;
    }
  | {
      type: "request_failed";
      requestGeneration: number;
      taskId?: string;
      error: BayesInferenceError;
      uncertain?: boolean;
    }
  | { type: "cancel_started"; requestGeneration: number; taskId: string };

export const initialBayesInferenceState: BayesInferenceState = {
  requestGeneration: 0,
  phase: "idle",
  task: null,
  result: null,
  error: null,
};

export function bayesInferenceReducer(
  state: BayesInferenceState,
  action: BayesInferenceAction,
): BayesInferenceState {
  if (action.requestGeneration !== state.requestGeneration && action.type !== "submit_started")
    return state;
  if ("taskId" in action && action.taskId && state.task?.taskId !== action.taskId) return state;

  switch (action.type) {
    case "submit_started":
      return {
        requestGeneration: action.requestGeneration,
        phase: "submitting",
        task: null,
        result: null,
        error: null,
      };
    case "task_received": {
      const task = action.task;
      if (state.task && state.task.taskId !== task.taskId) return state;
      if (
        ACTIVE_TASK_STATUSES.has(task.status) &&
        ["reading_result", "completed", "cancelled", "failed", "outcome_unknown"].includes(
          state.phase,
        )
      )
        return state;
      if (task.status === "failed")
        return { ...state, phase: "failed", task, error: task.error ?? defaultInferenceError() };
      if (task.status === "outcome_unknown")
        return { ...state, phase: "outcome_unknown", task, error: task.error };
      if (task.status === "cancelled") return { ...state, phase: "cancelled", task, error: null };
      if (task.status === "completed")
        return { ...state, phase: "reading_result", task, error: null };
      return { ...state, phase: "active", task, error: null };
    }
    case "result_received":
      if (action.result.artifactManifest.taskId !== action.taskId) return state;
      return { ...state, phase: "completed", result: action.result, error: null };
    case "request_failed":
      return {
        ...state,
        phase: action.uncertain ? "submission_unknown" : "failed",
        error: action.error,
      };
    case "cancel_started":
      return state.task && ACTIVE_TASK_STATUSES.has(state.task.status)
        ? { ...state, task: { ...state.task, status: "cancelling" } }
        : state;
  }
}

export function useBayesInferenceTask() {
  const [state, dispatch] = useReducer(bayesInferenceReducer, initialBayesInferenceState);
  const nextRequestGeneration = useRef(0);
  const submission = useRef<{
    draft: BayesModelDraftDTO;
    options: Parameters<typeof submitBayesInference>[1];
  } | null>(null);

  useEffect(() => {
    const taskId = state.task?.taskId;
    if (!taskId || state.phase !== "active") return;
    const requestGeneration = state.requestGeneration;
    let cancelled = false;
    const poll = () =>
      void getBayesInferenceStatus(taskId)
        .then((task) => {
          if (!cancelled) dispatch({ type: "task_received", requestGeneration, task });
        })
        .catch((caught: unknown) => {
          if (!cancelled)
            dispatch({
              type: "request_failed",
              requestGeneration,
              taskId,
              error: formatBayesError(caught),
              uncertain: true,
            });
        });
    const intervalId = window.setInterval(poll, 1_000);
    poll();
    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [state.phase, state.requestGeneration, state.task?.taskId]);

  useEffect(() => {
    const taskId = state.task?.taskId;
    if (!taskId || state.phase !== "reading_result") return;
    const requestGeneration = state.requestGeneration;
    let cancelled = false;
    void readBayesInferenceResult(taskId)
      .then((result) => {
        if (!cancelled) dispatch({ type: "result_received", requestGeneration, taskId, result });
      })
      .catch((caught: unknown) => {
        if (!cancelled)
          dispatch({
            type: "request_failed",
            requestGeneration,
            taskId,
            error: formatBayesError(caught),
            uncertain: true,
          });
      });
    return () => {
      cancelled = true;
    };
  }, [state.phase, state.requestGeneration, state.task?.taskId]);

  const submit = async (attempt: NonNullable<typeof submission.current>) => {
    const requestGeneration = ++nextRequestGeneration.current;
    dispatch({ type: "submit_started", requestGeneration });
    try {
      const task = await submitBayesInference(attempt.draft, attempt.options);
      dispatch({ type: "task_received", requestGeneration, task });
    } catch (caught) {
      const error = formatBayesError(caught);
      const uncertain = [
        "bayes_request_failed",
        "plugin_request_timeout",
        "plugin_request_failed",
        "plugin_process_exited",
        "plugin_response_invalid",
        "plugin_outcome_unknown",
      ].includes(error.code);
      dispatch({ type: "request_failed", requestGeneration, error, uncertain });
    }
  };

  const run = async (draft: BayesModelDraftDTO, timeoutMs: number) => {
    submission.current = {
      draft: structuredClone(draft),
      options: { operationId: createOperationId(), timeoutMs },
    };
    await submit(submission.current);
  };
  const retrySubmission = async () => {
    if (state.phase === "submission_unknown" && submission.current)
      await submit(submission.current);
  };

  const cancel = () => {
    const taskId = state.task?.taskId;
    if (!taskId || !ACTIVE_TASK_STATUSES.has(state.task!.status)) return;
    const requestGeneration = state.requestGeneration;
    dispatch({ type: "cancel_started", requestGeneration, taskId });
    void cancelBayesInference(taskId).catch((caught: unknown) =>
      dispatch({
        type: "request_failed",
        requestGeneration,
        taskId,
        error: formatBayesError(caught),
        uncertain: true,
      }),
    );
  };

  return {
    task: state.task,
    result: state.result,
    error: state.error,
    phase: state.phase,
    run,
    cancel,
    retrySubmission,
  };
}

function defaultInferenceError(): BayesInferenceError {
  return {
    code: "bayes_inference_failed",
    details: null,
    incidentId: null,
  };
}

function formatBayesError(caught: unknown): BayesInferenceError {
  return normalizeBayesApplicationError(caught, "bayes_request_failed");
}

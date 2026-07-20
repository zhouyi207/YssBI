import { useEffect, useReducer, useRef } from 'react';
import type { BayesInferenceTaskDTO, BayesModelDraftDTO, InferenceResultDTO, TaskErrorDTO } from '@/shared/types/bayes';
import { cancelBayesInference, getBayesInferenceStatus, readBayesInferenceResult, submitBayesInference } from '@/services/bayes';

const ACTIVE_TASK_STATUSES = new Set<BayesInferenceTaskDTO['status']>(['queued', 'running', 'cancelling']);

interface TauriAppErrorLike {
  code?: string;
  message?: string;
  details?: { column?: string | null; row?: number | null } | null;
}

export interface BayesInferenceError extends TaskErrorDTO {
  column?: string;
  row?: number;
}

export interface BayesInferenceState {
  runId: number;
  phase: 'idle' | 'submitting' | 'active' | 'reading_result' | 'completed' | 'cancelled' | 'failed';
  task: BayesInferenceTaskDTO | null;
  result: InferenceResultDTO | null;
  error: BayesInferenceError | null;
}

type BayesInferenceAction =
  | { type: 'submit_started'; runId: number }
  | { type: 'task_received'; runId: number; task: BayesInferenceTaskDTO }
  | { type: 'result_received'; runId: number; taskId: string; result: InferenceResultDTO }
  | { type: 'request_failed'; runId: number; taskId?: string; error: BayesInferenceError }
  | { type: 'cancel_started'; runId: number; taskId: string };

export const initialBayesInferenceState: BayesInferenceState = {
  runId: 0,
  phase: 'idle',
  task: null,
  result: null,
  error: null,
};

export function bayesInferenceReducer(state: BayesInferenceState, action: BayesInferenceAction): BayesInferenceState {
  if (action.runId !== state.runId && action.type !== 'submit_started') return state;
  if ('taskId' in action && action.taskId && state.task?.taskId !== action.taskId) return state;

  switch (action.type) {
    case 'submit_started':
      return { runId: action.runId, phase: 'submitting', task: null, result: null, error: null };
    case 'task_received': {
      const task = action.task;
      if (state.task && state.task.taskId !== task.taskId) return state;
      if (ACTIVE_TASK_STATUSES.has(task.status) && ['reading_result', 'completed', 'cancelled', 'failed'].includes(state.phase)) return state;
      if (task.status === 'failed') return { ...state, phase: 'failed', task, error: task.error ?? defaultInferenceError() };
      if (task.status === 'cancelled') return { ...state, phase: 'cancelled', task, error: null };
      if (task.status === 'completed') return { ...state, phase: 'reading_result', task, error: null };
      return { ...state, phase: 'active', task, error: null };
    }
    case 'result_received':
      if (action.result.artifactManifest.taskId !== action.taskId) return state;
      return { ...state, phase: 'completed', result: action.result, error: null };
    case 'request_failed':
      return { ...state, phase: 'failed', error: action.error };
    case 'cancel_started':
      return state.task && ACTIVE_TASK_STATUSES.has(state.task.status)
        ? { ...state, task: { ...state.task, status: 'cancelling' } }
        : state;
  }
}

export function useBayesInferenceTask() {
  const [state, dispatch] = useReducer(bayesInferenceReducer, initialBayesInferenceState);
  const nextRunId = useRef(0);

  useEffect(() => {
    const taskId = state.task?.taskId;
    if (!taskId || state.phase !== 'active') return;
    const runId = state.runId;
    let cancelled = false;
    const poll = () => void getBayesInferenceStatus(taskId)
      .then(task => { if (!cancelled) dispatch({ type: 'task_received', runId, task }); })
      .catch((caught: unknown) => { if (!cancelled) dispatch({ type: 'request_failed', runId, taskId, error: formatBayesError(caught) }); });
    const intervalId = window.setInterval(poll, 1_000);
    poll();
    return () => { cancelled = true; window.clearInterval(intervalId); };
  }, [state.phase, state.runId, state.task?.taskId]);

  useEffect(() => {
    const taskId = state.task?.taskId;
    if (!taskId || state.phase !== 'reading_result') return;
    const runId = state.runId;
    let cancelled = false;
    void readBayesInferenceResult(taskId)
      .then(result => { if (!cancelled) dispatch({ type: 'result_received', runId, taskId, result }); })
      .catch((caught: unknown) => { if (!cancelled) dispatch({ type: 'request_failed', runId, taskId, error: formatBayesError(caught) }); });
    return () => { cancelled = true; };
  }, [state.phase, state.runId, state.task?.taskId]);

  const run = async (draft: BayesModelDraftDTO) => {
    const runId = ++nextRunId.current;
    dispatch({ type: 'submit_started', runId });
    try {
      const task = await submitBayesInference(draft);
      dispatch({ type: 'task_received', runId, task });
    } catch (caught) {
      dispatch({ type: 'request_failed', runId, error: formatBayesError(caught) });
    }
  };

  const cancel = () => {
    const taskId = state.task?.taskId;
    if (!taskId || !ACTIVE_TASK_STATUSES.has(state.task!.status)) return;
    const runId = state.runId;
    dispatch({ type: 'cancel_started', runId, taskId });
    void cancelBayesInference(taskId)
      .catch((caught: unknown) => dispatch({ type: 'request_failed', runId, taskId, error: formatBayesError(caught) }));
  };

  return { task: state.task, result: state.result, error: state.error, phase: state.phase, run, cancel };
}

function defaultInferenceError(): BayesInferenceError {
  return { code: 'BAYES_INFERENCE_FAILED', message: 'Bayesian inference failed.' };
}

function formatBayesError(caught: unknown): BayesInferenceError {
  if (caught instanceof Error) return { code: 'BAYES_REQUEST_FAILED', message: caught.message };
  if (isTauriAppErrorLike(caught)) {
    return {
      code: caught.code ?? 'BAYES_REQUEST_FAILED',
      message: caught.message ?? 'Bayesian inference failed.',
      column: caught.details?.column ?? undefined,
      row: caught.details?.row ?? undefined,
    };
  }
  if (typeof caught === 'string') return { code: 'BAYES_REQUEST_FAILED', message: caught };
  return { code: 'BAYES_REQUEST_FAILED', message: String(caught) };
}

function isTauriAppErrorLike(value: unknown): value is TauriAppErrorLike {
  return typeof value === 'object' && value !== null && ('message' in value || 'code' in value);
}

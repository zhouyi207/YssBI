import { useEffect, useMemo, useRef, useState } from 'react';
import type { InferenceResultDTO } from '@/shared/types/bayes';

interface AsyncDataState<T> {
  data: T | null;
  loading: boolean;
  error: unknown | null;
}

export function useBayesPlotData<T>(
  result: InferenceResultDTO | null,
  load: (taskId: string, parameter: string) => Promise<T>,
) {
  const taskId = posteriorSamplesTaskId(result);
  const parameters = useMemo(() => result?.summaries.map(summary => summary.parameter) ?? [], [result]);
  const [selection, setSelection] = useState<{ taskId: string; parameter: string } | null>(null);
  const parameter = selectParameterForTask(taskId, parameters, selection);
  const state = useAsyncData(taskId && parameter ? `${taskId}:${parameter}` : null, () => load(taskId!, parameter!));
  const setSelectedParameter = (nextParameter: string) => {
    if (taskId) setSelection({ taskId, parameter: nextParameter });
  };

  return { ...state, parameters, parameter, setSelectedParameter };
}

function useAsyncData<T>(key: string | null, load: () => Promise<T>): AsyncDataState<T> {
  const [state, setState] = useState<AsyncDataState<T>>({ data: null, loading: false, error: null });
  const loadRef = useRef(load);
  loadRef.current = load;

  useEffect(() => {
    let stale = false;
    setState({ data: null, loading: Boolean(key), error: null });
    if (!key) return;

    void loadRef.current()
      .then(data => { if (!stale) setState({ data, loading: false, error: null }); })
      .catch((caught: unknown) => {
        if (!stale) setState({ data: null, loading: false, error: caught });
      });
    return () => { stale = true; };
  }, [key]);

  return state;
}

export function selectParameterForTask(
  taskId: string | undefined,
  parameters: readonly string[],
  selection: { taskId: string; parameter: string } | null,
): string | undefined {
  if (taskId && selection?.taskId === taskId && parameters.includes(selection.parameter)) return selection.parameter;
  return parameters[0];
}

function posteriorSamplesTaskId(result: InferenceResultDTO | null): string | undefined {
  return result?.artifactManifest.artifacts.some(artifact => artifact.kind === 'posterior_samples')
    ? result.artifactManifest.taskId
    : undefined;
}

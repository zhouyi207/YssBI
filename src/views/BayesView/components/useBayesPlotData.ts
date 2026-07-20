import { useEffect, useMemo, useState } from 'react';
import type { InferenceResultDTO } from '@/shared/types/bayes';

interface AsyncDataState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
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

  useEffect(() => {
    let stale = false;
    setState({ data: null, loading: Boolean(key), error: null });
    if (!key) return;

    void load()
      .then(data => { if (!stale) setState({ data, loading: false, error: null }); })
      .catch((caught: unknown) => {
        if (!stale) setState({ data: null, loading: false, error: caught instanceof Error ? caught.message : String(caught) });
      });
    return () => { stale = true; };
  }, [key, load]);

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

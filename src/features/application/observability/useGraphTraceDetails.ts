import { useCallback, useEffect, useRef, useState } from 'react';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { TraceService } from '@/services/nodeSystem/traceService';
import type { TraceDecimalString, TraceRecordDto } from '@/shared/types/dto/trace';

export interface TraceQueryError {
  code: string;
  message: string;
}

export interface GraphTraceDetailsState {
  graphTraces: TraceRecordDto[];
  graphLoading: boolean;
  graphError: TraceQueryError | null;
  selectedRunId: TraceDecimalString | null;
  runTrace: TraceRecordDto[];
  runLoading: boolean;
  runError: TraceQueryError | null;
  selectedRunNotFound: boolean;
  refresh(): Promise<void>;
  selectRun(runId: TraceDecimalString | null): Promise<void>;
}

export function useGraphTraceDetails(graphPath: string): GraphTraceDetailsState {
  const activeProjectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [graphTraces, setGraphTraces] = useState<TraceRecordDto[]>([]);
  const [graphLoading, setGraphLoading] = useState(false);
  const [graphError, setGraphError] = useState<TraceQueryError | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<TraceDecimalString | null>(null);
  const [runTrace, setRunTrace] = useState<TraceRecordDto[]>([]);
  const [runLoading, setRunLoading] = useState(false);
  const [runError, setRunError] = useState<TraceQueryError | null>(null);
  const graphRequestGeneration = useRef(0);
  const runRequestGeneration = useRef(0);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    const requestGeneration = ++graphRequestGeneration.current;
    setGraphLoading(true);
    setGraphError(null);

    let context: ReturnType<typeof captureProjectCommandContext>;
    try {
      context = captureProjectCommandContext();
    } catch (caught) {
      if (mounted.current && graphRequestGeneration.current === requestGeneration) {
        setGraphLoading(false);
        setGraphError(toTraceQueryError(caught));
      }
      return;
    }

    try {
      const traces = await TraceService.listGraphTraces(context.projectInstanceId, graphPath);
      const completion = classifyQueryCompletion(
        mounted.current,
        requestGeneration,
        graphRequestGeneration.current,
        context.isCurrent(),
      );
      if (completion !== 'current') {
        if (completion === 'staleProject') setGraphLoading(false);
        return;
      }
      setGraphTraces(traces);
      setGraphLoading(false);
    } catch (caught) {
      const completion = classifyQueryCompletion(
        mounted.current,
        requestGeneration,
        graphRequestGeneration.current,
        context.isCurrent(),
      );
      if (completion !== 'current') {
        if (completion === 'staleProject') setGraphLoading(false);
        return;
      }
      setGraphLoading(false);
      setGraphError(toTraceQueryError(caught));
    }
  }, [activeProjectInstanceId, graphPath]);

  const selectRun = useCallback(async (runId: TraceDecimalString | null) => {
    const requestGeneration = ++runRequestGeneration.current;
    setSelectedRunId(runId);
    setRunTrace([]);
    setRunError(null);

    if (runId === null) {
      setRunLoading(false);
      return;
    }

    setRunLoading(true);
    let context: ReturnType<typeof captureProjectCommandContext>;
    try {
      context = captureProjectCommandContext();
    } catch (caught) {
      if (mounted.current && runRequestGeneration.current === requestGeneration) {
        setRunLoading(false);
        setRunError(toTraceQueryError(caught));
      }
      return;
    }

    try {
      const traces = await TraceService.getRunTrace(context.projectInstanceId, runId);
      const completion = classifyQueryCompletion(
        mounted.current,
        requestGeneration,
        runRequestGeneration.current,
        context.isCurrent(),
      );
      if (completion !== 'current') {
        if (completion === 'staleProject') setRunLoading(false);
        return;
      }
      setRunTrace(traces);
      setRunLoading(false);
    } catch (caught) {
      const completion = classifyQueryCompletion(
        mounted.current,
        requestGeneration,
        runRequestGeneration.current,
        context.isCurrent(),
      );
      if (completion !== 'current') {
        if (completion === 'staleProject') setRunLoading(false);
        return;
      }
      setRunLoading(false);
      setRunError(toTraceQueryError(caught));
    }
  }, []);

  useEffect(() => {
    ++runRequestGeneration.current;
    setGraphTraces([]);
    setGraphError(null);
    setSelectedRunId(null);
    setRunTrace([]);
    setRunLoading(false);
    setRunError(null);
    void refresh();
  }, [refresh]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      ++graphRequestGeneration.current;
      ++runRequestGeneration.current;
    };
  }, []);

  return {
    graphTraces,
    graphLoading,
    graphError,
    selectedRunId,
    runTrace,
    runLoading,
    runError,
    selectedRunNotFound: runError?.code === 'trace_not_found',
    refresh,
    selectRun,
  };
}

type QueryCompletion = 'current' | 'staleProject' | 'ignored';

function classifyQueryCompletion(
  isMounted: boolean,
  requestGeneration: number,
  currentGeneration: number,
  isCurrentProject: boolean,
): QueryCompletion {
  if (!isMounted || requestGeneration !== currentGeneration) return 'ignored';
  return isCurrentProject ? 'current' : 'staleProject';
}

function toTraceQueryError(caught: unknown): TraceQueryError {
  if (typeof caught === 'object' && caught !== null) {
    const error = caught as { code?: unknown; message?: unknown };
    return {
      code: typeof error.code === 'string' ? error.code : 'trace_query_failed',
      message: typeof error.message === 'string' ? error.message : 'Unable to load trace details.',
    };
  }
  if (typeof caught === 'string') {
    return { code: 'trace_query_failed', message: caught };
  }
  return { code: 'trace_query_failed', message: 'Unable to load trace details.' };
}

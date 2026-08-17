import { useCallback, useEffect, useRef, useState } from 'react';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { TraceService } from '@/services/nodeSystem/traceService';
import { IpcError } from '@/services/ipc';
import type {
  CompilationTraceBundleDto,
  RunTraceBundleDto,
  TraceBundleDto,
  TraceDecimalString,
  TraceSpanDto,
} from '@/shared/types/dto/trace';

export interface TraceSpanProjection extends TraceSpanDto {
  durationNanos: bigint;
}

export interface CompilationTraceBundleProjection
  extends Omit<CompilationTraceBundleDto, 'spans'> {
  spans: TraceSpanProjection[];
}

export interface RunTraceBundleProjection extends Omit<RunTraceBundleDto, 'spans'> {
  spans: TraceSpanProjection[];
}

export type TraceBundleProjection =
  | CompilationTraceBundleProjection
  | RunTraceBundleProjection;

export interface TraceQueryError {
  code: string;
  message: string;
}

export interface GraphTraceDetailsState {
  graphBundles: TraceBundleProjection[];
  graphLoading: boolean;
  graphError: TraceQueryError | null;
  selectedRunId: TraceDecimalString | null;
  runBundle: RunTraceBundleProjection | null;
  runLoading: boolean;
  runError: TraceQueryError | null;
  selectedRunNotFound: boolean;
  refresh(): Promise<void>;
  selectRun(runId: TraceDecimalString | null): Promise<void>;
}

export function useGraphTraceDetails(graphPath: string): GraphTraceDetailsState {
  const activeProjectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [graphBundles, setGraphBundles] = useState<TraceBundleProjection[]>([]);
  const [graphLoading, setGraphLoading] = useState(false);
  const [graphError, setGraphError] = useState<TraceQueryError | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<TraceDecimalString | null>(null);
  const [runBundle, setRunBundle] = useState<RunTraceBundleProjection | null>(null);
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
      const bundles = await TraceService.listGraphTraceBundles(
        context.projectInstanceId,
        graphPath,
      );
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
      setGraphBundles(bundles.map(projectTraceBundle));
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
    setRunBundle(null);
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
      const bundle = await TraceService.getRunTraceBundle(context.projectInstanceId, runId);
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
      setRunBundle(projectRunTraceBundle(bundle));
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
    setGraphBundles([]);
    setGraphError(null);
    setSelectedRunId(null);
    setRunBundle(null);
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
    graphBundles,
    graphLoading,
    graphError,
    selectedRunId,
    runBundle,
    runLoading,
    runError,
    selectedRunNotFound: runError?.code === 'trace_not_found',
    refresh,
    selectRun,
  };
}

export function projectTraceSpan(span: TraceSpanDto): TraceSpanProjection {
  return {
    ...span,
    durationNanos: BigInt(span.finishedAt) - BigInt(span.startedAt),
  };
}

export function projectTraceBundle(bundle: TraceBundleDto): TraceBundleProjection {
  if (bundle.bundleKind === 'run') return projectRunTraceBundle(bundle);
  return {
    ...bundle,
    spans: bundle.spans.map(projectTraceSpan),
  };
}

function projectRunTraceBundle(bundle: RunTraceBundleDto): RunTraceBundleProjection {
  return {
    ...bundle,
    spans: bundle.spans.map(projectTraceSpan),
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
  if (caught instanceof IpcError) {
    return { code: caught.code, message: 'Unable to load trace details.' };
  }
  return { code: 'trace_query_failed', message: 'Unable to load trace details.' };
}

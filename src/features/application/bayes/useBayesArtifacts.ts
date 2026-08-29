import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  exportBayesArtifactCsv,
  readBayesAutocorrelationData,
  readBayesDensityPlotData,
  readBayesPosteriorPredictive,
  readBayesTracePlotData,
} from '@/services/bayes';
import { revealPath } from '@/services/platform/opener';
import { savePathDialog } from '@/services/platform/pathDialog';
import type { PlatformFailure } from '@/services/platform/platformTypes';
import { toErrorReference, type ErrorReference } from '@/features/application/errorReference';
import { databaseRead } from '@/features/application/dataManagement/databaseRead';
import { useDatabaseRead } from '@/features/application/dataManagement/databaseRead';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { freezeProjectionSnapshot, type DeepReadonly } from '@/shared/types/deepReadonly';
import type {
  AutocorrelationPlotDataDTO,
  DensityPlotDataDTO,
  InferenceResultDTO,
  PosteriorPredictivePageDTO,
  TracePlotDataDTO,
} from '@/shared/types/bayes';

export type BayesTracePlotData = TracePlotDataDTO;
export type BayesDensityPlotData = DensityPlotDataDTO;
export type BayesAutocorrelationData = AutocorrelationPlotDataDTO;
export type BayesPosteriorPredictive = PosteriorPredictivePageDTO;

export interface BayesArtifactsOptions {
  readonly result: InferenceResultDTO | null;
  readonly artifactPath?: string;
  readonly exportKind?: 'posterior_samples' | 'posterior_predictive';
  readonly exportFileName?: string;
  readonly exportDialogTitle?: string;
}

export type BayesReadOutcome<T> =
  | { readonly status: 'ready'; readonly value: DeepReadonly<T> }
  | { readonly status: 'stale' | 'notReady' | 'failed' };

export type BayesArtifactActionOutcome =
  | { readonly status: 'completed' | 'cancelled' | 'stale' | 'failed' };

export interface BayesArtifactsModel {
  readonly loading: boolean;
  readonly issue: ErrorReference | null;
  readonly parameters: readonly string[];
  readonly parameter: string | undefined;
  readonly setSelectedParameter: (parameter: string) => void;
  readonly readTrace: () => Promise<BayesReadOutcome<BayesTracePlotData>>;
  readonly readDensity: () => Promise<BayesReadOutcome<BayesDensityPlotData>>;
  readonly readAutocorrelation: () => Promise<BayesReadOutcome<BayesAutocorrelationData>>;
  readonly readPosteriorPredictive: () => Promise<BayesReadOutcome<BayesPosteriorPredictive>>;
  readonly exportCsv: () => Promise<BayesArtifactActionOutcome>;
  readonly revealResultFolder: () => Promise<BayesArtifactActionOutcome>;
}

interface RequestGeneration {
  readonly request: number;
  readonly identity: ProjectIdentitySnapshot;
  readonly databaseGeneration: string;
  readonly taskId: string | null;
  readonly parameter: string | undefined;
}

export function useBayesArtifacts({
  result,
  artifactPath: providedArtifactPath,
  exportKind,
  exportFileName = 'bayes-result.csv',
  exportDialogTitle = 'Export Bayes result',
}: BayesArtifactsOptions): BayesArtifactsModel {
  const databaseRevisions = useDatabaseRead((snapshot) => snapshot.revisions);
  const databaseGeneration = useMemo(
    () => revisionGeneration(databaseRevisions),
    [databaseRevisions],
  );
  const taskId = result?.artifactManifest.taskId ?? null;
  const parameters = useMemo(
    () => result?.summaries.map((summary) => summary.parameter) ?? [],
    [result],
  );
  const [selection, setSelection] = useState<{ taskId: string; parameter: string } | null>(null);
  const [loading, setLoading] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);
  const nextRequest = useRef(0);
  const mounted = useRef(true);
  const taskIdRef = useRef(taskId);
  const parameterRef = useRef<string | undefined>(undefined);
  taskIdRef.current = taskId;

  const parameter = selectParameter(taskId, parameters, selection);
  parameterRef.current = parameter;

  useEffect(() => () => {
    mounted.current = false;
    nextRequest.current += 1;
  }, []);

  useEffect(() => {
    setIssue(null);
    setLoading(false);
  }, [taskId]);

  const begin = useCallback((): RequestGeneration | null => {
    let identity: ProjectIdentitySnapshot;
    try {
      identity = captureProjectIdentity();
    } catch {
      return null;
    }
    const request: RequestGeneration = {
      request: ++nextRequest.current,
      identity,
      databaseGeneration,
      taskId: taskIdRef.current,
      parameter: parameterRef.current,
    };
    setLoading(true);
    setIssue(null);
    return request;
  }, [databaseGeneration]);

  const isCurrent = useCallback((request: RequestGeneration): boolean => (
    mounted.current
    && request.request === nextRequest.current
    && request.taskId === taskIdRef.current
    && request.parameter === parameterRef.current
    && request.databaseGeneration === revisionGeneration(databaseRead.getSnapshot().revisions)
    && isCurrentProjectIdentity(request.identity)
  ), []);

  const finish = useCallback((request: RequestGeneration) => {
    if (isCurrent(request)) setLoading(false);
  }, [isCurrent]);

  const read = useCallback(async <T,>(
    fallbackCode: string,
    operation: () => Promise<T>,
  ): Promise<BayesReadOutcome<T>> => {
    const request = begin();
    if (!request) return { status: 'notReady' };
    try {
      const value = await operation();
      if (!isCurrent(request)) return { status: 'stale' };
      return { status: 'ready', value: freezeProjectionSnapshot(value) };
    } catch (error) {
      if (!isCurrent(request)) return { status: 'stale' };
      setIssue(toErrorReference(error, fallbackCode));
      return { status: 'failed' };
    } finally {
      finish(request);
    }
  }, [begin, finish, isCurrent]);

  const readTrace = useCallback(() => {
    if (!taskId || !parameter || !hasArtifact(result, 'posterior_samples')) {
      return Promise.resolve<BayesReadOutcome<BayesTracePlotData>>({ status: 'notReady' });
    }
    return read('bayes_trace_read_failed', () => readBayesTracePlotData(taskId, parameter, 500));
  }, [parameter, read, result, taskId]);

  const readDensity = useCallback(() => {
    if (!taskId || !parameter || !hasArtifact(result, 'posterior_samples')) {
      return Promise.resolve<BayesReadOutcome<BayesDensityPlotData>>({ status: 'notReady' });
    }
    return read('bayes_density_read_failed', () => readBayesDensityPlotData(taskId, parameter, 256));
  }, [parameter, read, result, taskId]);

  const readAutocorrelation = useCallback(() => {
    if (!taskId || !parameter || !hasArtifact(result, 'posterior_samples')) {
      return Promise.resolve<BayesReadOutcome<BayesAutocorrelationData>>({ status: 'notReady' });
    }
    return read('bayes_autocorrelation_read_failed', () => readBayesAutocorrelationData(taskId, parameter, 50));
  }, [parameter, read, result, taskId]);

  const readPosteriorPredictive = useCallback(() => {
    if (!result || !taskId || !hasArtifact(result, 'posterior_predictive')) {
      return Promise.resolve<BayesReadOutcome<BayesPosteriorPredictive>>({ status: 'notReady' });
    }
    const rows = findArtifact(result, 'posterior_predictive')?.rows;
    return read(
      'bayes_posterior_predictive_read_failed',
      () => readBayesPosteriorPredictive(taskId, 0, Math.max(rows ?? 10_000, 1)),
    );
  }, [read, result, taskId]);

  const exportCsv = useCallback(async (): Promise<BayesArtifactActionOutcome> => {
    if (!taskId || !exportKind || !hasArtifact(result, exportKind)) {
      return { status: 'failed' };
    }
    const request = begin();
    if (!request) return { status: 'failed' };
    try {
      const selection = await savePathDialog({
        title: exportDialogTitle,
        defaultPath: exportFileName,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!isCurrent(request)) return { status: 'stale' };
      if (!selection.ok) {
        setIssue(platformIssue(selection.failure, 'bayes_export_path_failed'));
        return { status: 'failed' };
      }
      if (selection.value === null) return { status: 'cancelled' };

      await exportBayesArtifactCsv(taskId, exportKind, selection.value);
      if (!isCurrent(request)) return { status: 'stale' };
      return { status: 'completed' };
    } catch (error) {
      if (!isCurrent(request)) return { status: 'stale' };
      setIssue(toErrorReference(error, 'bayes_export_failed'));
      return { status: 'failed' };
    } finally {
      finish(request);
    }
  }, [begin, exportDialogTitle, exportFileName, exportKind, finish, isCurrent, result, taskId]);

  const revealResultFolder = useCallback(async (): Promise<BayesArtifactActionOutcome> => {
    const artifactPath = providedArtifactPath ?? result?.artifactManifest.artifacts[0]?.path;
    if (!artifactPath) return { status: 'failed' };
    const request = begin();
    if (!request) return { status: 'failed' };
    try {
      const outcome = await revealPath(artifactPath);
      if (!isCurrent(request)) return { status: 'stale' };
      if (!outcome.ok) {
        setIssue(platformIssue(outcome.failure, 'bayes_result_reveal_failed'));
        return { status: 'failed' };
      }
      return { status: 'completed' };
    } catch (error) {
      if (!isCurrent(request)) return { status: 'stale' };
      setIssue(toErrorReference(error, 'bayes_result_reveal_failed'));
      return { status: 'failed' };
    } finally {
      finish(request);
    }
  }, [begin, finish, isCurrent, providedArtifactPath, result]);

  const setSelectedParameter = useCallback((nextParameter: string) => {
    if (taskId && parameters.includes(nextParameter)) {
      setSelection({ taskId, parameter: nextParameter });
    }
  }, [parameters, taskId]);

  return useMemo(() => ({
    loading,
    issue,
    parameters,
    parameter,
    setSelectedParameter,
    readTrace,
    readDensity,
    readAutocorrelation,
    readPosteriorPredictive,
    exportCsv,
    revealResultFolder,
  }), [
    exportCsv,
    issue,
    loading,
    parameter,
    parameters,
    readAutocorrelation,
    readDensity,
    readPosteriorPredictive,
    readTrace,
    revealResultFolder,
    setSelectedParameter,
  ]);
}

export function selectParameter(
  taskId: string | null | undefined,
  parameters: readonly string[],
  selection: { readonly taskId: string; readonly parameter: string } | null,
): string | undefined {
  if (taskId && selection?.taskId === taskId && parameters.includes(selection.parameter)) {
    return selection.parameter;
  }
  return parameters[0];
}

function hasArtifact(
  result: InferenceResultDTO | null,
  kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind'],
): boolean {
  return result?.artifactManifest.artifacts.some((artifact) => artifact.kind === kind) ?? false;
}

function findArtifact(
  result: InferenceResultDTO,
  kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind'],
) {
  return result.artifactManifest.artifacts.find((artifact) => artifact.kind === kind);
}

function revisionGeneration(revisions: Readonly<Record<string, number>>): string {
  return JSON.stringify(Object.entries(revisions).sort(([left], [right]) => left.localeCompare(right)));
}

function platformIssue(failure: PlatformFailure, fallbackCode: string): ErrorReference {
  return {
    code: fallbackCode,
    incidentId: failure.incidentId ?? null,
  };
}

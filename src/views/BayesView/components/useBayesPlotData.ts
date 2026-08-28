import { useEffect, useRef, useState } from 'react';
import type { InferenceResultDTO } from '@/shared/types/bayes';
import {
  useBayesArtifacts,
  type BayesArtifactsModel,
  type BayesAutocorrelationData,
  type BayesDensityPlotData,
  type BayesReadOutcome,
  type BayesTracePlotData,
} from '@/features/application/bayes';

export interface BayesPlotDataState<T> {
  readonly data: BayesPlotValue<T> | null;
  readonly loading: boolean;
  readonly error: BayesArtifactsModel['issue'];
  readonly parameters: readonly string[];
  readonly parameter: string | undefined;
  readonly setSelectedParameter: (parameter: string) => void;
}

type PlotReader<T> = (
  artifacts: BayesArtifactsModel,
) => Promise<BayesReadOutcome<T>>;

type BayesPlotValue<T> = Extract<BayesReadOutcome<T>, { readonly status: 'ready' }>['value'];

const readTrace: PlotReader<BayesTracePlotData> = (artifacts) => artifacts.readTrace();
const readDensity: PlotReader<BayesDensityPlotData> = (artifacts) => artifacts.readDensity();
const readAutocorrelation: PlotReader<BayesAutocorrelationData> = (artifacts) => artifacts.readAutocorrelation();

export function useBayesPlotData<T>(
  result: InferenceResultDTO | null,
  reader: PlotReader<T>,
): BayesPlotDataState<T> {
  const artifacts = useBayesArtifacts({ result });
  const [data, setData] = useState<BayesPlotValue<T> | null>(null);
  const artifactsRef = useRef(artifacts);
  const readerRef = useRef(reader);
  artifactsRef.current = artifacts;
  readerRef.current = reader;

  useEffect(() => {
    setData(null);
    let current = true;
    void readerRef.current(artifactsRef.current).then((outcome) => {
      if (!current || outcome.status !== 'ready') return;
      setData(outcome.value);
    });
    return () => {
      current = false;
    };
  }, [artifacts.parameter, reader, result]);

  return {
    data,
    loading: artifacts.loading,
    error: artifacts.issue,
    parameters: artifacts.parameters,
    parameter: artifacts.parameter,
    setSelectedParameter: artifacts.setSelectedParameter,
  };
}

export function useBayesTracePlotData(
  result: InferenceResultDTO | null,
): BayesPlotDataState<BayesTracePlotData> {
  return useBayesPlotData(result, readTrace);
}

export function useBayesDensityPlotData(
  result: InferenceResultDTO | null,
): BayesPlotDataState<BayesDensityPlotData> {
  return useBayesPlotData(result, readDensity);
}

export function useBayesAutocorrelationData(
  result: InferenceResultDTO | null,
): BayesPlotDataState<BayesAutocorrelationData> {
  return useBayesPlotData(result, readAutocorrelation);
}

export { selectParameter as selectParameterForTask } from '@/features/application/bayes/useBayesArtifacts';

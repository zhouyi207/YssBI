import { invoke } from '@tauri-apps/api/core';
import type {
  AutocorrelationPlotDataDTO,
  BayesInferenceTaskDTO,
  BayesModelDraftDTO,
  DensityPlotDataDTO,
  InferenceResultDTO,
  PosteriorPredictivePageDTO,
  PosteriorSamplePageDTO,
  TracePlotDataDTO,
} from '@/shared/types/bayes';

export async function submitBayesInference(input: BayesModelDraftDTO): Promise<BayesInferenceTaskDTO> {
  return invoke<BayesInferenceTaskDTO>('submit_bayes_inference', { input });
}

export async function getBayesInferenceStatus(taskId: string): Promise<BayesInferenceTaskDTO> {
  return invoke<BayesInferenceTaskDTO>('get_bayes_inference_status', { taskId });
}

export async function cancelBayesInference(taskId: string): Promise<void> {
  await invoke('cancel_bayes_inference', { taskId });
}

export async function readBayesInferenceResult(taskId: string): Promise<InferenceResultDTO> {
  return invoke<InferenceResultDTO>('read_bayes_inference_result', { taskId });
}

export async function clearBayesInferenceTask(taskId: string): Promise<void> {
  await invoke('clear_bayes_inference_task', { taskId });
}

export async function readBayesPosteriorSamples(
  taskId: string,
  offset: number,
  limit: number,
  parameter?: string,
): Promise<PosteriorSamplePageDTO> {
  return invoke<PosteriorSamplePageDTO>('read_bayes_posterior_samples', {
    taskId,
    offset,
    limit,
    parameter: parameter ?? null,
  });
}

export async function readBayesTracePlotData(
  taskId: string,
  parameter?: string,
  maxPointsPerChain = 500,
): Promise<TracePlotDataDTO> {
  return invoke<TracePlotDataDTO>('read_bayes_trace_plot_data', {
    taskId,
    parameter: parameter ?? null,
    maxPointsPerChain,
  });
}

export async function readBayesDensityPlotData(
  taskId: string,
  parameter?: string,
  bins = 64,
): Promise<DensityPlotDataDTO> {
  return invoke<DensityPlotDataDTO>('read_bayes_density_plot_data', {
    taskId,
    parameter: parameter ?? null,
    bins,
  });
}

export async function readBayesAutocorrelationData(
  taskId: string,
  parameter?: string,
  maxLag = 50,
): Promise<AutocorrelationPlotDataDTO> {
  return invoke<AutocorrelationPlotDataDTO>('read_bayes_autocorrelation_data', {
    taskId,
    parameter: parameter ?? null,
    maxLag,
  });
}

export async function readBayesPosteriorPredictive(
  taskId: string,
  offset: number,
  limit: number,
): Promise<PosteriorPredictivePageDTO> {
  return invoke<PosteriorPredictivePageDTO>('read_bayes_posterior_predictive', {
    taskId,
    offset,
    limit,
  });
}

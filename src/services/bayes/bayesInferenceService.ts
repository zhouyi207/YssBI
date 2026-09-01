import { invokeCommand } from "@/services/ipc";
import { revealPath } from "@/services/platform/opener";
import {
  parseBayesInferenceTaskDTO,
  parseInferenceResultDTO,
} from "@/shared/types/bayes/wireParser";
import type {
  AutocorrelationPlotDataDTO,
  BayesInferenceTaskDTO,
  BayesModelDraftDTO,
  DensityPlotDataDTO,
  InferenceResultDTO,
  PosteriorPredictivePageDTO,
  TracePlotDataDTO,
} from "@/shared/types/bayes";

export async function submitBayesInference(
  input: BayesModelDraftDTO,
): Promise<BayesInferenceTaskDTO> {
  return parseBayesInferenceTaskDTO(
    await invokeCommand<unknown>("submit_bayes_inference", { input }),
  );
}

export async function getBayesInferenceStatus(taskId: string): Promise<BayesInferenceTaskDTO> {
  return parseBayesInferenceTaskDTO(
    await invokeCommand<unknown>("get_bayes_inference_status", { taskId }),
  );
}

export async function cancelBayesInference(taskId: string): Promise<void> {
  await invokeCommand("cancel_bayes_inference", { taskId });
}

export async function readBayesInferenceResult(taskId: string): Promise<InferenceResultDTO> {
  return parseInferenceResultDTO(
    await invokeCommand<unknown>("read_bayes_inference_result", { taskId }),
  );
}

export async function revealBayesResultFolder(artifactPath: string): Promise<void> {
  const result = await revealPath(artifactPath);
  if (!result.ok) throw new Error(result.failure.code);
}

export async function exportBayesArtifactCsv(
  taskId: string,
  kind: "posterior_samples" | "posterior_predictive",
  destination: string,
): Promise<void> {
  await invokeCommand("export_bayes_artifact_csv", { taskId, kind, destination });
}

export async function readBayesTracePlotData(
  taskId: string,
  parameter?: string,
  maxPointsPerChain = 500,
): Promise<TracePlotDataDTO> {
  return invokeCommand<TracePlotDataDTO>("read_bayes_trace_plot_data", {
    taskId,
    parameter: parameter ?? null,
    maxPointsPerChain,
  });
}

export async function readBayesDensityPlotData(
  taskId: string,
  parameter?: string,
  gridPoints = 256,
): Promise<DensityPlotDataDTO> {
  return invokeCommand<DensityPlotDataDTO>("read_bayes_density_plot_data", {
    taskId,
    parameter: parameter ?? null,
    gridPoints,
  });
}

export async function readBayesAutocorrelationData(
  taskId: string,
  parameter?: string,
  maxLag = 50,
): Promise<AutocorrelationPlotDataDTO> {
  return invokeCommand<AutocorrelationPlotDataDTO>("read_bayes_autocorrelation_data", {
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
  return invokeCommand<PosteriorPredictivePageDTO>("read_bayes_posterior_predictive", {
    taskId,
    offset,
    limit,
  });
}

import { request } from "@/sdk";
import { invokeCommand } from "@/services/ipc";
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

function parseTaskSnapshot(value: unknown): BayesInferenceTaskDTO {
  if (value === null || typeof value !== "object") {
    throw new Error("Invalid Bayes inference task response");
  }
  const snapshot = value as Record<string, unknown>;
  const statuses: Record<string, BayesInferenceTaskDTO["status"]> = {
    admitted: "queued",
    running: "running",
    cancelRequested: "cancelling",
    succeeded: "completed",
    failed: "failed",
    cancelled: "cancelled",
    outcomeUnknown: "outcome_unknown",
  };
  return parseBayesInferenceTaskDTO({
    taskId: snapshot.taskId,
    status: typeof snapshot.state === "string" ? statuses[snapshot.state] : undefined,
    progress: snapshot.progress ?? null,
    error: snapshot.error,
  });
}

export async function submitBayesInference(
  input: BayesModelDraftDTO,
  options: { operationId: string; timeoutMs: number },
): Promise<BayesInferenceTaskDTO> {
  return parseTaskSnapshot(
    await request("tasks.start", {
      taskType: "bayes.inference",
      operationId: options.operationId,
      parameters: input,
      timeoutMs: options.timeoutMs,
    }),
  );
}

export async function getBayesInferenceStatus(taskId: string): Promise<BayesInferenceTaskDTO> {
  return parseTaskSnapshot(await request("tasks.get", { taskId }));
}

export async function cancelBayesInference(taskId: string): Promise<void> {
  await request("tasks.cancel", { taskId });
}

export async function readBayesInferenceResult(taskId: string): Promise<InferenceResultDTO> {
  return parseInferenceResultDTO(await request("tasks.result", { taskId }));
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

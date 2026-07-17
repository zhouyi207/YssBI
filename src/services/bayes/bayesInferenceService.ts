import { invoke } from '@tauri-apps/api/core';
import type {
  BayesInferenceTaskDTO,
  BayesModelDraftDTO,
  InferenceResultDTO,
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

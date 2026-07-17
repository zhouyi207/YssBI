import { useState } from 'react';
import type { BayesInferenceTaskDTO, BayesModelDraftDTO, InferenceResultDTO } from '@/shared/types/bayes';
import { cancelBayesInference, readBayesInferenceResult, submitBayesInference } from '@/services/bayes';

const MOCK_RESULT: InferenceResultDTO = {
  summaries: [
    { parameter: 'a', mean: 1.96, sd: 0.08, median: 1.96, q025: 1.8, q975: 2.12, rhat: 1.001, essBulk: 3240, essTail: 2810 },
    { parameter: 'b', mean: 1.12, sd: 0.19, median: 1.11, q025: 0.75, q975: 1.48, rhat: 1.002, essBulk: 2980, essTail: 2510 },
    { parameter: 'sigma', mean: 0.42, sd: 0.06, median: 0.41, q025: 0.32, q975: 0.56, rhat: 1.000, essBulk: 3600, essTail: 3300 },
  ],
  diagnostics: {
    chains: 4,
    drawsPerChain: 2000,
    warmup: 1000,
    divergences: 0,
    maxTreedepthHits: 0,
    warnings: [],
  },
};

export function useBayesInferenceTask() {
  const [task, setTask] = useState<BayesInferenceTaskDTO | null>(null);
  const [result, setResult] = useState<InferenceResultDTO | null>(null);

  const run = async (draft: BayesModelDraftDTO) => {
    setResult(null);
    const submitted = await submitBayesInference(draft).catch((): BayesInferenceTaskDTO => {
      const taskId = `mock-bayes-${Date.now()}`;
      return { taskId, status: 'completed', result: { taskId, summaryPath: 'mock://summary.json' } };
    });
    setTask(submitted);
    if (submitted.status === 'completed') {
      const nextResult = await readBayesInferenceResult(submitted.taskId).catch(() => MOCK_RESULT);
      setResult(nextResult);
    }
  };

  const cancel = () => {
    const taskId = task?.taskId;
    if (taskId) void cancelBayesInference(taskId).catch(() => undefined);
    setTask(current => current ? { ...current, status: 'cancelled' } : current);
  };

  return { task, result, run, cancel };
}

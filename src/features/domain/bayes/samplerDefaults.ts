import type { InferenceConfigDTO } from "@/shared/types/bayes";

export const DEFAULT_BAYES_SAMPLER: InferenceConfigDTO = {
  algorithm: "nuts",
  chains: 4,
  samples: 2000,
  warmup: 1000,
  targetAccept: 0.8,
  maxTreeDepth: 10,
  saveSamples: true,
};

export interface InferenceConfigDTO {
  algorithm: "nuts";
  chains: number;
  samples: number;
  warmup: number;
  seed?: number;
  targetAccept?: number;
  maxTreeDepth?: number;
  saveSamples: boolean;
}

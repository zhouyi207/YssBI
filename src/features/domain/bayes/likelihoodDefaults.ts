import type { LikelihoodSpecDTO } from '@/shared/types/bayes';

export const DEFAULT_NORMAL_LIKELIHOOD: LikelihoodSpecDTO = {
  type: 'normal',
  mean: { source: 'predictor' },
  sigma: { parameter: 'sigma' },
};

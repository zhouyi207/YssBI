export type LikelihoodSpecDTO =
  | {
      type: 'normal';
      mean: { source: 'predictor' };
      sigma: { parameter: string };
    }
  | {
      type: 'bernoulli_logit';
      logit: { source: 'predictor' };
    }
  | {
      type: 'poisson_log';
      logRate: { source: 'predictor' };
    };

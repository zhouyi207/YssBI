import type { BayesModelDraftDTO, BayesModelSpecDTO } from '@/shared/types/bayes';
import { DEFAULT_NORMAL_LIKELIHOOD } from './likelihoodDefaults';
import { DEFAULT_BAYES_SAMPLER } from './samplerDefaults';

export function createEmptyBayesDraft(): BayesModelDraftDTO {
  return {
    formulaText: '',
    responseSymbol: undefined,
    rawPredictor: null,
    symbols: [],
    dataset: null,
    responseBinding: null,
    dataBindings: {},
    boundPredictor: null,
    likelihood: DEFAULT_NORMAL_LIKELIHOOD,
    parameters: [],
    sampler: DEFAULT_BAYES_SAMPLER,
  };
}

export function draftToModelSpec(draft: BayesModelDraftDTO): BayesModelSpecDTO | null {
  if (!draft.responseBinding || !draft.boundPredictor) return null;
  return {
    responseColumn: draft.responseBinding.column,
    responseSymbol: draft.responseSymbol,
    predictor: draft.boundPredictor,
    dataVariables: draft.dataBindings,
    likelihood: draft.likelihood,
    parameters: draft.parameters,
  };
}

export function hashBayesDraft(draft: BayesModelDraftDTO): string {
  return JSON.stringify({
    formulaText: draft.formulaText,
    responseSymbol: draft.responseSymbol,
    predictor: draft.boundPredictor,
    rawPredictor: draft.rawPredictor,
    symbols: draft.symbols,
    dataset: draft.dataset?.sourceId ?? null,
    responseBinding: draft.responseBinding,
    dataBindings: draft.dataBindings,
    likelihood: draft.likelihood,
    parameters: draft.parameters,
    sampler: draft.sampler,
  });
}

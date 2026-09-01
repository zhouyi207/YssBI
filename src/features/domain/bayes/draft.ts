import type { BayesModelDraftDTO, RawExpressionDTO } from "@/shared/types/bayes";
import {
  bindRawExpression,
  bindResponseExpression,
  collectRawSymbols,
  createSymbolDrafts,
  symbolNamesByRole,
} from "./expressionSymbols";
import { DEFAULT_NORMAL_LIKELIHOOD } from "./likelihoodDefaults";
import { likelihoodParameterNames, mergeInferredParameters } from "./parameterInference";
import { DEFAULT_BAYES_SAMPLER } from "./samplerDefaults";

export const DEFAULT_BAYES_FORMULA =
  "y \\sim \\operatorname{Normal}\\left(a \\cdot x + b, \\sigma\\right)";

const DEFAULT_RESPONSE: RawExpressionDTO = { type: "symbol", name: "y" };
const DEFAULT_PREDICTOR: RawExpressionDTO = {
  type: "binary",
  op: "add",
  left: {
    type: "binary",
    op: "mul",
    left: { type: "symbol", name: "a" },
    right: { type: "symbol", name: "x" },
  },
  right: { type: "symbol", name: "b" },
};

export function createEmptyBayesDraft(): BayesModelDraftDTO {
  return {
    formulaText: "",
    rawResponse: { type: "symbol", name: "y" },
    rawPredictor: null,
    symbols: [],
    dataset: null,
    responseBinding: null,
    dataBindings: {},
    boundResponse: { type: "data_variable", name: "y" },
    boundPredictor: null,
    likelihood: DEFAULT_NORMAL_LIKELIHOOD,
    parameters: [],
    sampler: DEFAULT_BAYES_SAMPLER,
  };
}

export function createDefaultBayesDraft(): BayesModelDraftDTO {
  const draft = createEmptyBayesDraft();
  const symbolNames = [
    ...collectRawSymbols(DEFAULT_RESPONSE),
    ...collectRawSymbols(DEFAULT_PREDICTOR),
    ...likelihoodParameterNames(draft.likelihood),
  ];
  const symbols = createSymbolDrafts(symbolNames, [], []);
  const parameters = mergeInferredParameters(
    [],
    symbolNamesByRole(symbols, "parameter"),
    draft.likelihood,
  ).parameters;

  return {
    ...draft,
    formulaText: DEFAULT_BAYES_FORMULA,
    rawResponse: DEFAULT_RESPONSE,
    rawPredictor: DEFAULT_PREDICTOR,
    symbols,
    boundResponse: bindResponseExpression(DEFAULT_RESPONSE),
    boundPredictor: bindRawExpression(DEFAULT_PREDICTOR, symbols),
    parameters,
  };
}

export function hashBayesDraft(draft: BayesModelDraftDTO): string {
  return JSON.stringify({
    formulaText: draft.formulaText,
    response: draft.boundResponse,
    rawResponse: draft.rawResponse,
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

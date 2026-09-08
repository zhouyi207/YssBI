import type {
  BayesModelDraftDTO,
  LikelihoodSpecDTO,
  ParseExpressionRequestDTO,
} from "@/shared/types/bayes";
import { likelihoodParameterNames } from "@/features/domain/bayes";
import { normalizeBayesApplicationError, type BayesApplicationError } from "./bayesError";

export type FormulaParseError = BayesApplicationError;

export function buildFormulaParseRequest(
  draft: BayesModelDraftDTO,
  formula: string,
  likelihood: LikelihoodSpecDTO,
): ParseExpressionRequestDTO {
  const symbols = [
    ...draft.symbols.map((symbol) => symbol.name),
    ...(draft.dataset?.columns.map((column) => column.name) ?? []),
    ...likelihoodParameterNames(likelihood),
  ].filter((name, index, names) => name.length > 0 && names.indexOf(name) === index);

  return {
    formula,
    columns: draft.dataset?.columns,
    symbols,
  };
}

export function formatFormulaParseError(caught: unknown): FormulaParseError {
  return normalizeBayesApplicationError(caught, "bayes_expression_parse_failed");
}

import type {
  BayesModelDraftDTO,

  LikelihoodSpecDTO,
  ParseExpressionRequestDTO,
} from '@/shared/types/bayes';
import { likelihoodParameterNames } from '@/features/domain/bayes';

export interface FormulaParseError {
  code: string;
  message: string;
  detail?: string;
}



export function buildFormulaParseRequest(
  draft: BayesModelDraftDTO,
  formula: string,
  likelihood: LikelihoodSpecDTO,
): ParseExpressionRequestDTO {
  const symbols = [
    ...draft.symbols.map(symbol => symbol.name),
    ...(draft.dataset?.columns.map(column => column.name) ?? []),
    ...likelihoodParameterNames(likelihood),
  ].filter((name, index, names) => name.length > 0 && names.indexOf(name) === index);

  return {
    formula,
    columns: draft.dataset?.columns,
    symbols,
  };
}

export function restoreParsedSymbols(
  deletedSymbols: ReadonlySet<string>,
  parsedSymbols: readonly string[],
): Set<string> {
  const next = new Set(deletedSymbols);
  parsedSymbols.forEach(symbol => next.delete(symbol));
  return next;
}


export function formatFormulaParseError(caught: unknown): FormulaParseError {
  if (typeof caught === 'object' && caught !== null) {
    const error = caught as { code?: unknown; message?: unknown; detail?: unknown; details?: unknown };
    const detail = typeof error.detail === 'string'
      ? error.detail
      : typeof error.details === 'string' ? error.details : undefined;
    return {
      code: typeof error.code === 'string' ? error.code : 'FORMULA_PARSE_FAILED',
      message: typeof error.message === 'string' ? error.message : 'Unable to parse the model formula',
      detail,
    };
  }
  return {
    code: 'FORMULA_PARSE_FAILED',
    message: typeof caught === 'string' ? caught : 'Unable to parse the model formula',
  };
}

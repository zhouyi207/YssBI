import type {
  BayesModelDraftDTO,

  LikelihoodSpecDTO,
  RawExpressionDTO,
  ParseExpressionRequestDTO,
  ParseExpressionResponseDTO,
} from '@/shared/types/bayes';
import { likelihoodParameterNames } from '@/features/domain/bayes';

export interface FormulaParseError {
  code: string;
  message: string;
  detail?: string;
}

interface EditableFormulaDraft {
  formulaText: string;
  rawResponse: RawExpressionDTO | null;
  rawPredictor: RawExpressionDTO | null;
}

export interface FormulaParseState {
  generation: number;
  formula: EditableFormulaDraft;
  error: FormulaParseError | null;
}

export type FormulaParseAction =
  | { type: 'started'; generation: number; formulaText: string }
  | { type: 'succeeded'; generation: number; response: ParseExpressionResponseDTO }
  | { type: 'failed'; generation: number; error: FormulaParseError };

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


export function formulaParseReducer(state: FormulaParseState, action: FormulaParseAction): FormulaParseState {
  if (action.type === 'started' && action.generation <= state.generation) return state;
  if (action.type !== 'started' && action.generation !== state.generation) return state;

  switch (action.type) {
    case 'started':
      return {
        generation: action.generation,
        formula: { formulaText: action.formulaText, rawResponse: null, rawPredictor: null },
        error: null,
      };
    case 'succeeded':
      return { generation: state.generation, formula: action.response.formula, error: null };
    case 'failed':
      return { ...state, error: action.error };
  }
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

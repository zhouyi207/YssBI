import { useMemo, useRef, useState } from 'react';
import type {
  BayesModelDraftDTO,
  BayesSymbolRoleDTO,
  InferenceConfigDTO,
  LikelihoodSpecDTO,
  ParameterSpecDTO,
  PriorSpecDTO,
  SymbolDraftDTO,
} from '@/shared/types/bayes';
import { parseBayesExpression } from '@/services/bayes';
import {
  bindRawExpression,
  bindResponseExpression,
  collectRawSymbols,
  createDefaultBayesDraft,
  createDefaultParameter,
  createSymbolDrafts,
  hashBayesDraft,
  likelihoodParameterNames,
  mergeInferredParameters,
  responseBaseNameFromRaw,
  symbolNamesByRole,
} from '@/features/domain/bayes';
import {
  buildFormulaParseRequest,
  formatFormulaParseError,
  formulaParseReducer,
  restoreParsedSymbols,
  type FormulaParseError,
  type FormulaParseState,
} from './formulaParsing';

export function useBayesModelDraft(initialDraft: BayesModelDraftDTO = createDefaultBayesDraft()) {
  const [draft, setDraft] = useState<BayesModelDraftDTO>(initialDraft);
  const [unusedParameterNames, setUnusedParameterNames] = useState<string[]>([]);
  const [deletedSymbolNames, setDeletedSymbolNames] = useState<Set<string>>(() => new Set());
  const [formulaError, setFormulaError] = useState<FormulaParseError | null>(null);
  const formulaRequestGeneration = useRef(0);

  const draftHash = useMemo(() => hashBayesDraft(draft), [draft]);

  const updateModelEquation = async (formulaText: string, likelihood: LikelihoodSpecDTO) => {
    const generation = ++formulaRequestGeneration.current;
    const request = buildFormulaParseRequest(draft, formulaText, likelihood);
    setFormulaError(null);
    setDraft(current => {
      const parsing = formulaParseReducer(formulaState(current, generation - 1, null), {
        type: 'started',
        generation,
        formulaText,
      });
      return { ...current, formulaText: parsing.formula.formulaText, likelihood };
    });

    try {
      const response = await parseBayesExpression(request);
      if (generation !== formulaRequestGeneration.current) return;
      setDeletedSymbolNames(currentDeleted => {
        const nextDeleted = restoreParsedSymbols(currentDeleted, response.symbols);
        setDraft(current => {
          const parsed = formulaParseReducer(formulaState(current, generation, null), {
            type: 'succeeded',
            generation,
            response,
          });
          return rebuildDraft(applyParsedFormula(current, parsed.formula), nextDeleted);
        });
        return nextDeleted;
      });
    } catch (caught) {
      if (generation !== formulaRequestGeneration.current) return;
      const error = formatFormulaParseError(caught);
      const failed = formulaParseReducer(formulaStateFromText(formulaText, generation), {
        type: 'failed',
        generation,
        error,
      });
      setFormulaError(failed.error);
    }
  };

  const updateSymbolRole = (name: string, role: BayesSymbolRoleDTO) => {
    setDraft(current => rebuildDraft(applySymbolRole(current, name, role)));
  };

  const updateSymbolDataBinding = (name: string, column: string) => {
    setDraft(current => rebuildDraft(applySymbolDataBinding(current, name, column)));
  };

  const updateSymbolPrior = (name: string, prior: PriorSpecDTO) => {
    setDraft(current => ({
      ...current,
      parameters: current.parameters.map(parameter => parameter.name === name ? { ...parameter, prior } : parameter),
    }));
  };

  const updateSymbolConstraint = (name: string, constraint: ParameterSpecDTO['constraint']) => {
    setDraft(current => ({
      ...current,
      parameters: current.parameters.map(parameter => parameter.name === name ? { ...parameter, constraint } : parameter),
    }));
  };

  const updateDataset = (dataset: BayesModelDraftDTO['dataset']) => {
    formulaRequestGeneration.current += 1;
    setDraft(current => rebuildDraft(applyDatasetDefaults({ ...current, dataset })));
  };

  const updateSymbolConfiguration = (configuration: {
    name: string;
    dataset: BayesModelDraftDTO['dataset'];
    role: BayesSymbolRoleDTO;
    column: string;
    constraint: ParameterSpecDTO['constraint'];
    prior: PriorSpecDTO;
  }) => {
    formulaRequestGeneration.current += 1;
    setDraft(current => {
      let next = applyDatasetDefaults({ ...current, dataset: configuration.dataset });
      next = applySymbolRole(next, configuration.name, configuration.role);
      next = applySymbolDataBinding(next, configuration.name, configuration.column);
      next = rebuildDraft(next);
      if (configuration.role !== 'parameter') return next;
      return {
        ...next,
        parameters: next.parameters.map(parameter => parameter.name === configuration.name
          ? { ...parameter, constraint: configuration.constraint, prior: configuration.prior }
          : parameter),
      };
    });
  };

  const deleteSymbol = (name: string) => {
    formulaRequestGeneration.current += 1;
    setDeletedSymbolNames(currentDeleted => {
      const nextDeleted = new Set(currentDeleted);
      nextDeleted.add(name);
      setDraft(current => rebuildDraft(removeSymbol(current, name), nextDeleted));
      return nextDeleted;
    });
  };

  const updateSymbols = (symbols: SymbolDraftDTO[]) => {
    formulaRequestGeneration.current += 1;
    setDraft(current => rebuildDraft({ ...current, symbols }));
  };

  const updateLikelihood = (likelihood: LikelihoodSpecDTO) => {
    formulaRequestGeneration.current += 1;
    setDraft(current => rebuildDraft({ ...current, likelihood }));
  };

  const updateParameters = (parameters: ParameterSpecDTO[]) => {
    setDraft(current => ({ ...current, parameters }));
  };

  const updateSampler = (sampler: InferenceConfigDTO) => {
    setDraft(current => ({ ...current, sampler }));
  };

  const rebuildDraft = (next: BayesModelDraftDTO, deletedNames: Set<string> = deletedSymbolNames): BayesModelDraftDTO => {
    const responseName = responseBaseNameFromRaw(next.rawResponse);
    const rawSymbols = [
      ...collectRawSymbols(next.rawResponse),
      ...collectRawSymbols(next.rawPredictor),
      ...likelihoodParameterNames(next.likelihood),
    ]
      .filter((name, index, names) => names.indexOf(name) === index)
      .filter(name => !deletedNames.has(name));
    const datasetColumns = next.dataset?.columns.map(column => column.name) ?? [];
    const likelihoodParameters = new Set(likelihoodParameterNames(next.likelihood));
    const symbols = createSymbolDrafts(rawSymbols, next.symbols, datasetColumns)
      .filter(symbol => !deletedNames.has(symbol.name))
      .map(symbol => {
        if (symbol.name === responseName) {
          return { ...symbol, role: 'dependent' as const, inferredRole: 'dependent' as const };
        }
        if (likelihoodParameters.has(symbol.name)) {
          return { ...symbol, role: 'parameter' as const, inferredRole: 'parameter' as const };
        }
        return symbol.role === 'dependent'
          ? { ...symbol, role: 'independent' as const }
          : symbol;
      });
    const boundResponse = bindResponseExpression(next.rawResponse);
    const boundPredictor = bindRawExpression(next.rawPredictor, symbols);
    const merged = mergeInferredParameters(next.parameters, symbolNamesByRole(symbols, 'parameter'), next.likelihood);
    setUnusedParameterNames(merged.unusedParameterNames);
    const independentSymbols = new Set(symbolNamesByRole(symbols, 'independent'));
    const dataBindings = Object.fromEntries(
      Object.entries(next.dataBindings).filter(([name]) => independentSymbols.has(name)),
    );
    const dependentSymbols = new Set(symbolNamesByRole(symbols, 'dependent'));
    const responseBinding = next.responseBinding && dependentSymbols.has(next.responseBinding.symbol)
      ? next.responseBinding
      : null;
    return { ...next, symbols, boundResponse, boundPredictor, parameters: merged.parameters, dataBindings, responseBinding };
  };

  return {
    draft,
    draftHash,
    setDraft,
    updateModelEquation,
    updateSymbolRole,
    updateSymbolDataBinding,
    updateSymbolPrior,
    updateSymbolConstraint,
    updateDataset,
    updateSymbolConfiguration,
    deleteSymbol,
    updateSymbols,
    updateLikelihood,
    updateParameters,
    updateSampler,
    unusedParameterNames,
    formulaError,
  };
}

function formulaState(
  draft: BayesModelDraftDTO,
  generation: number,
  error: FormulaParseError | null,
): FormulaParseState {
  return {
    generation,
    formula: {
      formulaText: draft.formulaText,
      rawResponse: draft.rawResponse,
      rawPredictor: draft.rawPredictor,
    },
    error,
  };
}

function formulaStateFromText(formulaText: string, generation: number): FormulaParseState {
  return {
    generation,
    formula: { formulaText, rawResponse: null, rawPredictor: null },
    error: null,
  };
}

function applyParsedFormula(
  draft: BayesModelDraftDTO,
  formula: FormulaParseState['formula'],
): BayesModelDraftDTO {
  if (!formula.rawResponse) return draft;
  const responseName = responseBaseNameFromRaw(formula.rawResponse);
  return {
    ...draft,
    formulaText: formula.formulaText,
    rawResponse: formula.rawResponse,
    rawPredictor: formula.rawPredictor,
    responseBinding: draft.responseBinding
      ? { ...draft.responseBinding, symbol: responseName }
      : { symbol: responseName, column: firstDatasetColumn(draft) ?? '' },
  };
}

function applySymbolRole(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): BayesModelDraftDTO {
  const symbols = draft.symbols.map(symbol => {
    if (symbol.name === name) return { ...symbol, role, userEdited: true };
    if (role === 'dependent' && symbol.role === 'dependent') {
      return { ...symbol, role: 'independent' as const, userEdited: true };
    }
    return symbol;
  });

  const next: BayesModelDraftDTO = {
    ...draft,
    symbols,
    responseBinding: role === 'dependent'
      ? { symbol: name, column: draft.responseBinding?.column ?? firstDatasetColumn(draft) ?? '' }
      : draft.responseBinding?.symbol === name ? null : draft.responseBinding,
  };

  if (role === 'parameter') {
    const { [name]: _removed, ...dataBindings } = next.dataBindings;
    return {
      ...next,
      dataBindings,
      parameters: next.parameters.some(parameter => parameter.name === name)
        ? next.parameters
        : [...next.parameters, createDefaultParameter(name)],
    };
  }

  return {
    ...next,
    parameters: next.parameters.filter(parameter => parameter.name !== name),
  };
}

function applySymbolDataBinding(draft: BayesModelDraftDTO, name: string, column: string): BayesModelDraftDTO {
  const symbol = draft.symbols.find(item => item.name === name);
  if (symbol?.role === 'dependent') {
    return {
      ...draft,
      responseBinding: { symbol: name, column },
    };
  }

  if (symbol?.role === 'independent') {
    return {
      ...draft,
      dataBindings: { ...draft.dataBindings, [name]: column },
    };
  }

  return draft;
}

function applyDatasetDefaults(draft: BayesModelDraftDTO): BayesModelDraftDTO {
  const dataset = draft.dataset;
  if (!dataset) return { ...draft, responseBinding: null, dataBindings: {} };

  const dependentSymbol = responseBaseNameFromRaw(draft.rawResponse);
  const responseColumn = dependentSymbol
    ? preferredColumn(dataset, dependentSymbol, ['number', 'integer', 'boolean'])
    : null;
  const responseBinding = dependentSymbol && responseColumn
    ? { symbol: dependentSymbol, column: responseColumn }
    : draft.responseBinding;

  const dataBindings = { ...draft.dataBindings };
  for (const symbol of draft.symbols.filter(symbol => symbol.role === 'independent')) {
    if (dataBindings[symbol.name] && dataset.columns.some(column => column.name === dataBindings[symbol.name])) {
      continue;
    }
    const column = preferredColumn(dataset, symbol.name, ['number', 'integer']);
    if (column) dataBindings[symbol.name] = column;
  }

  return {
    ...draft,
    responseBinding,
    dataBindings,
  };
}

function preferredColumn(
  dataset: NonNullable<BayesModelDraftDTO['dataset']>,
  symbolName: string,
  allowedTypes: readonly string[],
): string | null {
  const exact = dataset.columns.find(column => column.name === symbolName && allowedTypes.includes(column.dtype));
  if (exact) return exact.name;
  const compatible = dataset.columns.find(column => allowedTypes.includes(column.dtype));
  return compatible?.name ?? dataset.columns[0]?.name ?? null;
}

function removeSymbol(draft: BayesModelDraftDTO, name: string): BayesModelDraftDTO {
  const { [name]: _removed, ...dataBindings } = draft.dataBindings;
  return {
    ...draft,
    responseBinding: responseBaseNameFromRaw(draft.rawResponse) === name ? null : draft.responseBinding,
    dataBindings,
    symbols: draft.symbols.filter(symbol => symbol.name !== name),
    parameters: draft.parameters.filter(parameter => parameter.name !== name),
  };
}

function firstDatasetColumn(draft: BayesModelDraftDTO): string | undefined {
  return draft.dataset?.columns[0]?.name;
}


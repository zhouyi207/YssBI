import { useMemo, useRef, useState } from 'react';
import type {
  BayesModelDraftDTO,
  BayesSymbolRoleDTO,
  InferenceConfigDTO,
  LikelihoodSpecDTO,
  ParameterSpecDTO,
  ParseExpressionResponseDTO,
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
  type FormulaParseError,
} from './formulaParsing';

export function useBayesModelDraft(initialDraft: BayesModelDraftDTO = createDefaultBayesDraft()) {
  const [draft, setDraft] = useState<BayesModelDraftDTO>(initialDraft);
  const [unusedParameterNames, setUnusedParameterNames] = useState<string[]>([]);
  const [formulaError, setFormulaError] = useState<FormulaParseError | null>(null);
  const formulaRequestGeneration = useRef(0);

  const draftHash = useMemo(() => hashBayesDraft(draft), [draft]);

  const updateModelEquation = async (formulaText: string, likelihood: LikelihoodSpecDTO): Promise<boolean> => {
    const generation = ++formulaRequestGeneration.current;
    const request = buildFormulaParseRequest(draft, formulaText, likelihood);
    setFormulaError(null);

    try {
      const response = await parseBayesExpression(request);
      if (generation !== formulaRequestGeneration.current) return false;
      setDraft(current => rebuildDraft(
        applyParsedFormula({ ...current, likelihood }, response.formula),
      ));
      return true;
    } catch (caught) {
      if (generation !== formulaRequestGeneration.current) return false;
      setFormulaError(formatFormulaParseError(caught));
      return false;
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

  const rebuildDraft = (next: BayesModelDraftDTO): BayesModelDraftDTO => {
    const responseName = responseBaseNameFromRaw(next.rawResponse);
    const rawSymbols = [
      ...collectRawSymbols(next.rawResponse),
      ...collectRawSymbols(next.rawPredictor),
      ...likelihoodParameterNames(next.likelihood),
    ].filter((name, index, names) => names.indexOf(name) === index);
    const datasetColumns = next.dataset?.columns.map(column => column.name) ?? [];
    const likelihoodParameters = new Set(likelihoodParameterNames(next.likelihood));
    const symbols = createSymbolDrafts(rawSymbols, next.symbols, datasetColumns)
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
    updateSymbols,
    updateLikelihood,
    updateParameters,
    updateSampler,
    unusedParameterNames,
    formulaError,
    clearFormulaError: () => setFormulaError(null),
  };
}

function applyParsedFormula(
  draft: BayesModelDraftDTO,
  formula: ParseExpressionResponseDTO['formula'],
): BayesModelDraftDTO {
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



function firstDatasetColumn(draft: BayesModelDraftDTO): string | undefined {
  return draft.dataset?.columns[0]?.name;
}


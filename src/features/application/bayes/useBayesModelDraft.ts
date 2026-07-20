import { useMemo, useState } from 'react';
import type {
  BayesModelDraftDTO,
  BayesSymbolRoleDTO,
  InferenceConfigDTO,
  LikelihoodSpecDTO,
  ParameterSpecDTO,
  PriorSpecDTO,
  RawExpressionDTO,
  SymbolDraftDTO,
} from '@/shared/types/bayes';
import { parseBayesExpression } from '@/services/bayes';
import {
  bindRawExpression,
  collectRawSymbols,
  createDefaultParameter,
  createEmptyBayesDraft,
  createSymbolDrafts,
  hashBayesDraft,
  likelihoodParameterNames,
  mergeInferredParameters,
  parseRawExpression,
  symbolNamesByRole,
} from '@/features/domain/bayes';

const EXAMPLE_RAW_EXPRESSION: RawExpressionDTO = {
  type: 'binary',
  op: 'add',
  left: {
    type: 'binary',
    op: 'mul',
    left: { type: 'symbol', name: 'a' },
    right: { type: 'symbol', name: 'x' },
  },
  right: { type: 'symbol', name: 'b' },
};

export function createMockBayesDraft(): BayesModelDraftDTO {
  const base = createEmptyBayesDraft();
  const rawPredictor = EXAMPLE_RAW_EXPRESSION;
  const symbolNames = ['y', ...collectRawSymbols(rawPredictor)];
  const symbols = createSymbolDrafts(symbolNames, [], []);
  const boundPredictor = bindRawExpression(rawPredictor, symbols);
  const merged = mergeInferredParameters([], symbolNamesByRole(symbols, 'parameter'), base.likelihood);

  return {
    ...base,
    formulaText: 'y \\sim \\operatorname{Normal}\\left(a \\cdot x + b, \\sigma\\right)',
    responseSymbol: 'y',
    rawPredictor,
    symbols,
    dataset: null,
    responseBinding: null,
    dataBindings: {},
    boundPredictor,
    parameters: merged.parameters,
  };
}

export function useBayesModelDraft(initialDraft: BayesModelDraftDTO = createMockBayesDraft()) {
  const [draft, setDraft] = useState<BayesModelDraftDTO>(initialDraft);
  const [unusedParameterNames, setUnusedParameterNames] = useState<string[]>([]);
  const [deletedSymbolNames, setDeletedSymbolNames] = useState<Set<string>>(() => new Set());

  const draftHash = useMemo(() => hashBayesDraft(draft), [draft]);

  const updateFormulaText = (formulaText: string) => {
    setDraft(current => ({ ...current, formulaText }));
  };

  const updateModelEquation = async (responseSymbol: string, formulaText: string, likelihood: LikelihoodSpecDTO, predictorText?: string) => {
    const rawPredictor = predictorText
      ? await parseBayesExpression({ formula: `${responseSymbol} = ${predictorText}` })
        .then(response => response.formula.rawPredictor ?? parseRawExpression(predictorText).expression)
        .catch(() => parseRawExpression(predictorText).expression)
      : null;
    setDraft(current => rebuildDraft(ensureDependentSymbol({
      ...current,
      responseSymbol,
      formulaText,
      rawPredictor: rawPredictor ?? current.rawPredictor,
      likelihood,
    }, responseSymbol)));
  };

  const updateFormula = (formulaText: string, rawPredictor: RawExpressionDTO | null, responseSymbol?: string) => {
    setDraft(current => rebuildDraft({ ...current, formulaText, rawPredictor, responseSymbol }));
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
    setDeletedSymbolNames(currentDeleted => {
      const nextDeleted = new Set(currentDeleted);
      nextDeleted.add(name);
      setDraft(current => rebuildDraft(removeSymbol(current, name), nextDeleted));
      return nextDeleted;
    });
  };

  const updateSymbols = (symbols: SymbolDraftDTO[]) => {
    setDraft(current => rebuildDraft({ ...current, symbols }));
  };

  const updateLikelihood = (likelihood: LikelihoodSpecDTO) => {
    setDraft(current => rebuildDraft({ ...current, likelihood }));
  };

  const updateParameters = (parameters: ParameterSpecDTO[]) => {
    setDraft(current => ({ ...current, parameters }));
  };

  const updateSampler = (sampler: InferenceConfigDTO) => {
    setDraft(current => ({ ...current, sampler }));
  };

  const rebuildDraft = (next: BayesModelDraftDTO, deletedNames: Set<string> = deletedSymbolNames): BayesModelDraftDTO => {
    const rawSymbols = [...collectRawSymbols(next.rawPredictor), ...likelihoodParameterNames(next.likelihood)]
      .filter((name, index, names) => names.indexOf(name) === index)
      .filter(name => !deletedNames.has(name));
    const datasetColumns = next.dataset?.columns.map(column => column.name) ?? [];
    const likelihoodParameters = new Set(likelihoodParameterNames(next.likelihood));
    const symbols = createSymbolDrafts(rawSymbols, next.symbols, datasetColumns)
      .filter(symbol => !deletedNames.has(symbol.name))
      .map(symbol => likelihoodParameters.has(symbol.name)
        ? { ...symbol, role: 'parameter' as const, inferredRole: 'parameter' as const }
        : symbol);
    const boundPredictor = bindRawExpression(next.rawPredictor, symbols);
    const merged = mergeInferredParameters(next.parameters, symbolNamesByRole(symbols, 'parameter'), next.likelihood);
    setUnusedParameterNames(merged.unusedParameterNames);
    const independentSymbols = new Set(symbolNamesByRole(symbols, 'independent'));
    const dataBindings = Object.fromEntries(
      Object.entries(next.dataBindings).filter(([name]) => independentSymbols.has(name)),
    );
    const dependentSymbols = new Set(symbolNamesByRole(symbols, 'dependent'));
    const responseBinding = next.responseBinding && (!next.responseBinding.symbol || dependentSymbols.has(next.responseBinding.symbol))
      ? next.responseBinding
      : null;
    return { ...next, symbols, boundPredictor, parameters: merged.parameters, dataBindings, responseBinding };
  };

  return {
    draft,
    draftHash,
    setDraft,
    updateFormulaText,
    updateModelEquation,
    updateFormula,
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
  };
}

function ensureDependentSymbol(draft: BayesModelDraftDTO, name: string): BayesModelDraftDTO {
  const trimmedName = name.trim();
  if (!trimmedName) return draft;
  const hasSymbol = draft.symbols.some(symbol => symbol.name === trimmedName);
  const symbols = draft.symbols.map(symbol => symbol.role === 'dependent'
    ? { ...symbol, role: 'independent' as const, userEdited: true }
    : symbol);
  return {
    ...draft,
    responseSymbol: trimmedName,
    symbols: hasSymbol
      ? symbols.map(symbol => symbol.name === trimmedName ? { ...symbol, role: 'dependent' as const, userEdited: true } : symbol)
      : [{ name: trimmedName, role: 'dependent', inferredRole: 'dependent', userEdited: true }, ...symbols],
    responseBinding: draft.responseBinding
      ? { ...draft.responseBinding, symbol: trimmedName }
      : { symbol: trimmedName, column: firstDatasetColumn(draft) ?? '' },
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
    responseSymbol: role === 'dependent' ? name : draft.responseSymbol === name ? undefined : draft.responseSymbol,
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
      responseSymbol: name,
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

  const dependentSymbol = draft.symbols.find(symbol => symbol.role === 'dependent')?.name ?? draft.responseSymbol;
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
    responseSymbol: draft.responseSymbol === name ? undefined : draft.responseSymbol,
    responseBinding: draft.responseBinding?.symbol === name ? null : draft.responseBinding,
    dataBindings,
    symbols: draft.symbols.filter(symbol => symbol.name !== name),
    parameters: draft.parameters.filter(parameter => parameter.name !== name),
  };
}

function firstDatasetColumn(draft: BayesModelDraftDTO): string | undefined {
  return draft.dataset?.columns[0]?.name;
}


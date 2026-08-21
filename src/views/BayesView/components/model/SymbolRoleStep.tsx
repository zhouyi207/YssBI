import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { BayesDatasetSelectionDTO, BayesModelDraftDTO, BayesSymbolRoleDTO, ParameterConstraintDTO, PriorSpecDTO, ValidationIssueDTO } from '@/shared/types/bayes';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { defaultPriorForConstraint, formatPrior } from '@/features/domain/bayes';
import { PanelTitle } from './BayesFields';
import { LatexInline, latexSymbol } from './LatexPresentation';
import { SymbolConfigDialog } from './SymbolConfigDialog';
import { defaultPriorArgs, isPriorCompatibleWithConstraint, numericColumns, priorFromParts } from './symbolConfigValues';
import type { BayesDatasetOption, Translation } from './types';

export function SymbolRoleStep({
  draft,
  datasets,
  onSymbolConfigurationChange,
  issues,
}: {
  draft: BayesModelDraftDTO;
  datasets: BayesDatasetOption[];
  issues: ValidationIssueDTO[];
  onSymbolConfigurationChange: (configuration: {
    name: string;
    dataset: BayesDatasetSelectionDTO | null;
    role: BayesSymbolRoleDTO;
    column: string;
    constraint: ParameterConstraintDTO;
    prior: PriorSpecDTO;
  }) => void;
}) {
  const { t } = useTranslation();
  const [editingSymbol, setEditingSymbol] = useState<string | null>(null);
  const [selectedDatasetId, setSelectedDatasetId] = useState('');
  const [role, setRole] = useState<BayesSymbolRoleDTO>('parameter');
  const [detailValue, setDetailValue] = useState('');
  const [constraint, setConstraint] = useState<ParameterConstraintDTO>({ type: 'real' });
  const [priorDistribution, setPriorDistribution] = useState<PriorSpecDTO['distribution']>('normal');
  const [priorArgs, setPriorArgs] = useState<string[]>(['0', '10']);

  const loadSymbolEditorState = (symbolName: string) => {
    const symbol = draft.symbols.find(item => item.name === symbolName);
    if (!symbol) return false;
    setSelectedDatasetId(draft.dataset?.sourceId ?? '');
    setRole(symbol.role);
    setDetailValue(symbolDetailValue(draft, symbol.name, symbol.role));
    const parameter = draft.parameters.find(item => item.name === symbol.name);
    setConstraint(parameter?.constraint ?? { type: 'real' });
    setPriorDistribution(parameter?.prior.distribution ?? 'normal');
    setPriorArgs((parameter?.prior.args ?? [0, 10]).map(String));
    return true;
  };

  const beginEdit = (symbolName: string) => {
    if (!loadSymbolEditorState(symbolName)) return;
    setEditingSymbol(symbolName);
  };

  const cancelEdit = () => {
    setEditingSymbol(null);
    setSelectedDatasetId('');
    setRole('parameter');
    setDetailValue('');
  };

  const saveSymbolChanges = (name: string) => {
    const dataset = datasets.find(item => item.sourceId === selectedDatasetId) ?? null;
    onSymbolConfigurationChange({
      name,
      dataset: dataset ? datasetSelectionFromOption(dataset) : null,
      role,
      column: detailValue,
      constraint,
      prior: priorFromParts(priorDistribution, priorArgs),
    });
    cancelEdit();
  };

  return (
    <Card>
      <CardHeader><PanelTitle title={t('bayes.symbols.title')} issues={issues} /></CardHeader>
      <CardContent className="space-y-3">
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>{t('bayes.symbols.symbol')}</TableHead><TableHead>{t('bayes.symbols.role')}</TableHead><TableHead>{t('bayes.symbols.data')}</TableHead><TableHead>{t('bayes.symbols.column')}</TableHead><TableHead>{t('bayes.symbols.prior')}</TableHead><TableHead>{t('bayes.symbols.bounds')}</TableHead><TableHead className="w-32">{t('bayes.symbols.actions')}</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {symbolsInDisplayOrder(draft.symbols).map(symbol => (
                <TableRow key={symbol.name}>
                  <TableCell><LatexInline formulaText={latexSymbol(symbol.name)} /></TableCell>
                  <TableCell>{roleLabel(symbol.role, t)}</TableCell>
                                    <TableCell>{dataSourceLabel(draft, symbol.role, datasets, t)}</TableCell>
                                    <TableCell className="font-mono">{columnLabel(draft, symbol.name, symbol.role, t)}</TableCell>
                                    <TableCell className="font-mono">{priorLabel(draft, symbol.name, symbol.role, t)}</TableCell>
                  <TableCell className="font-mono">{boundsLabel(draft, symbol.name)}</TableCell>
                  <TableCell>
                    <Button size="sm" variant="outline" onClick={() => beginEdit(symbol.name)}>{t('bayes.actions.edit')}</Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <SymbolConfigDialog
          open={editingSymbol !== null}
          datasets={datasets}
          symbol={editingSymbol}
          selectedDatasetId={selectedDatasetId}
          role={role}
          detailValue={detailValue}
          constraint={constraint}
          priorDistribution={priorDistribution}
          priorArgs={priorArgs}
          onDatasetChange={setSelectedDatasetId}
          onRoleChange={(nextRole) => {
            setRole(nextRole);
            if (editingSymbol) setDetailValue(defaultSymbolDetailValue(draft, editingSymbol, nextRole));
          }}
          onDetailValueChange={setDetailValue}
          onConstraintChange={(nextConstraint) => {
            setConstraint(nextConstraint);
            if (!isPriorCompatibleWithConstraint(priorDistribution, nextConstraint)) {
              const nextPrior = defaultPriorForConstraint(nextConstraint, editingSymbol ?? 'parameter');
              setPriorDistribution(nextPrior.distribution);
              setPriorArgs(nextPrior.args.map(String));
            }
          }}
          onPriorDistributionChange={(distribution) => {
            setPriorDistribution(distribution);
            setPriorArgs(defaultPriorArgs(distribution).map(String));
          }}
          onPriorArgsChange={setPriorArgs}
          onClose={cancelEdit}
          t={t}
          onSave={() => {
            if (!editingSymbol) return;
            saveSymbolChanges(editingSymbol);
          }}
        />
      </CardContent>
    </Card>
  );
}

function symbolsInDisplayOrder(symbols: readonly BayesModelDraftDTO['symbols'][number][]): BayesModelDraftDTO['symbols'] {
  const roleOrder: Record<BayesSymbolRoleDTO, number> = { dependent: 0, independent: 1, parameter: 2 };
  return symbols
    .map((symbol, index) => ({ symbol, index }))
    .sort((left, right) => roleOrder[left.symbol.role] - roleOrder[right.symbol.role] || left.index - right.index)
    .map(({ symbol }) => symbol);
}

function roleLabel(role: BayesSymbolRoleDTO, t: Translation): string {
  switch (role) {
    case 'dependent':
      return t('bayes.roles.dependent');
    case 'independent':
      return t('bayes.roles.independent');
    case 'parameter':
      return t('bayes.roles.parameter');
  }
}

function parameterForSymbol(draft: BayesModelDraftDTO, name: string) {
  return draft.parameters.find(parameter => parameter.name === name);
}

function boundsLabel(draft: BayesModelDraftDTO, name: string): string {
  const constraint = parameterForSymbol(draft, name)?.constraint;
  return constraint ? boundsSummary(constraint) : '—';
}

function boundsSummary(constraint: ParameterConstraintDTO): string {
  switch (constraint.type) {
    case 'real':
      return '(-∞, ∞)';
    case 'positive':
      return '(0, ∞)';
    case 'unit':
      return '(0, 1)';
    case 'bounded': {
      const left = constraint.includeLower ? '[' : '(';
      const right = constraint.includeUpper ? ']' : ')';
      return `${left}${constraint.lower}, ${constraint.upper}${right}`;
    }
  }
}

function symbolDetailValue(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role === 'dependent') return draft.responseBinding?.symbol === name ? draft.responseBinding.column : '';
  if (role === 'independent') return draft.dataBindings[name] ?? '';
  return draft.parameters.find(parameter => parameter.name === name)?.prior.distribution ?? 'normal';
}

function defaultSymbolDetailValue(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role === 'parameter') return 'normal';
  const columns = numericColumns(draft.dataset?.columns ?? []);
  const current = symbolDetailValue(draft, name, role);
  if (columns.some(column => column.name === current)) return current;
  return columns.find(column => column.name === name)?.name ?? columns[0]?.name ?? '';
}

function dataSourceLabel(
  draft: BayesModelDraftDTO,
  role: BayesSymbolRoleDTO,
  datasets: readonly BayesDatasetOption[],
  t: Translation,
): string {
  if (role === 'parameter') return '—';
  if (!draft.dataset) return t('bayes.dataBinding.noDataSelected');
  return datasets.find(dataset => dataset.sourceId === draft.dataset?.sourceId)?.displayName ?? t('bayes.dataBinding.unknownDataSource');
}

function columnLabel(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO, t: Translation): string {
  if (role === 'parameter') return '—';
  return symbolDetailValue(draft, name, role) || t('bayes.dataBinding.unboundColumn');
}

function priorLabel(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO, t: Translation): string {
  if (role !== 'parameter') return '—';
  const parameter = parameterForSymbol(draft, name);
  return parameter ? formatPrior(parameter.prior) : t('bayes.prior.notSet');
}

function datasetSelectionFromOption(option: BayesDatasetOption): BayesDatasetSelectionDTO {
  return {
    sourceType: option.sourceType,
    sourceId: option.sourceId,
    columns: option.columns,
  };
}

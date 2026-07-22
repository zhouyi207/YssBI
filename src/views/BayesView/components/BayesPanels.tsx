import { useEffect, useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { BayesDatasetSelectionDTO, BayesModelDraftDTO, BayesSymbolRoleDTO, InferenceConfigDTO, LikelihoodSpecDTO, ParameterConstraintDTO, PriorSpecDTO, ValidationIssueDTO } from '@/shared/types/bayes';
import type { FormulaParseError } from '@/features/application/bayes';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { defaultPriorForConstraint, formatExpression, formatPrior, formatRawExpressionLatex } from '@/features/domain/bayes';

export interface BayesDatasetOption extends BayesDatasetSelectionDTO {
  displayName: string;
}

export function FormulaStep({
  draft,
  onModelEquationChange,
  onErrorClear,
  error,
}: {
  draft: BayesModelDraftDTO;
  onModelEquationChange: (formulaText: string, likelihood: LikelihoodSpecDTO) => Promise<boolean>;
  onErrorClear: () => void;
  error: FormulaParseError | null;
}) {
  const [editing, setEditing] = useState(false);
  const [responseExpression, setResponseExpression] = useState(currentResponseExpression(draft));
  const [distribution, setDistribution] = useState<LikelihoodDistribution>(likelihoodDistribution(draft.likelihood));
  const [distributionArgs, setDistributionArgs] = useState<string[]>(() => initialDistributionArgs(draft));

  useEffect(() => {
    if (!editing) {
      setResponseExpression(currentResponseExpression(draft));
      setDistribution(likelihoodDistribution(draft.likelihood));
      setDistributionArgs(initialDistributionArgs(draft));
    }
  }, [draft, editing]);

  const applyDistribution = (nextDistribution: LikelihoodDistribution) => {
    setDistribution(nextDistribution);
    setDistributionArgs(currentArgs => resizeDistributionArgs(nextDistribution, currentArgs, draft));
  };

  const commit = async () => {
    const nextResponse = responseExpression.trim() || 'y';
    const nextFormulaText = composeLikelihoodLatex(nextResponse, distribution, distributionArgs);
    const saved = await onModelEquationChange(
      nextFormulaText,
      likelihoodFromFormulaParts(distribution, distributionArgs, draft.likelihood),
    );
    if (saved) setEditing(false);
  };

  const cancel = () => {
    setResponseExpression(currentResponseExpression(draft));
    setDistribution(likelihoodDistribution(draft.likelihood));
    setDistributionArgs(initialDistributionArgs(draft));
    onErrorClear();
    setEditing(false);
  };

  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-3">
        <PanelTitle title="1. Formula" issues={error ? [formulaErrorIssue(error)] : []} />
        <Button size="sm" variant="outline" onClick={() => setEditing(true)} disabled={editing}>
          编辑
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1.5">
          {editing ? (
            <div className="space-y-3 rounded-md border border-border bg-muted/20 p-3">
              <div className="grid gap-3 md:grid-cols-[120px_180px_minmax(0,1fr)]">
                <div className="space-y-1.5">
                  <Label htmlFor="bayes-response-expression" className="text-xs text-muted-foreground">响应表达式</Label>
                  <Input
                    id="bayes-response-expression"
                    value={responseExpression}
                    autoFocus
                    className="h-8 font-mono"
                    onChange={(event) => setResponseExpression(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Escape') {
                        event.preventDefault();
                        cancel();
                      }
                    }}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">分布</Label>
                  <Select value={distribution} onValueChange={(value) => applyDistribution(value as LikelihoodDistribution)}>
                    <SelectTrigger size="sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="normal">Normal</SelectItem>
                      <SelectItem value="bernoulli_logit">BernoulliLogit</SelectItem>
                      <SelectItem value="poisson_log">PoissonLog</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="grid gap-2 md:grid-cols-2">
                  {distributionArgLabels(distribution).map((label, index) => (
                    <div key={label} className="space-y-1.5">
                      <Label className="text-xs text-muted-foreground">{label}</Label>
                      <Input
                        value={distributionArgs[index] ?? ''}
                        className="h-8 font-mono"
                        onChange={(event) => setDistributionArgs(current => replaceAt(current, index, event.target.value))}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter' && event.ctrlKey) commit();
                          if (event.key === 'Escape') {
                            event.preventDefault();
                            cancel();
                          }
                        }}
                      />
                    </div>
                  ))}
                </div>
              </div>
              <LatexFormulaPreview formulaText={composeLikelihoodLatex(responseExpression || 'y', distribution, distributionArgs)} />
              <RecognizedSymbols symbols={draft.symbols.map(symbol => symbol.name)} />
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="outline" onClick={cancel}>取消</Button>
                <Button size="sm" onClick={commit}>保存</Button>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              <LatexFormulaPreview formulaText={draft.formulaText} />
              <RecognizedSymbols symbols={draft.symbols.map(symbol => symbol.name)} />
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

type LikelihoodDistribution = LikelihoodSpecDTO['type'];

export function currentResponseExpression(draft: BayesModelDraftDTO): string {
  return formatRawExpressionLatex(draft.rawResponse) || 'y';
}

function likelihoodDistribution(likelihood: LikelihoodSpecDTO): LikelihoodDistribution {
  return likelihood.type;
}

function initialDistributionArgs(draft: BayesModelDraftDTO): string[] {
  const predictor = extractPredictorLatex(draft.formulaText) || formatExpression(draft.boundPredictor) || 'a \\cdot x + b';
  switch (draft.likelihood.type) {
    case 'normal':
      return [predictor, latexSymbol(draft.likelihood.sigma.parameter)];
    case 'bernoulli_logit':
    case 'poisson_log':
      return [predictor];
  }
}

function resizeDistributionArgs(distribution: LikelihoodDistribution, currentArgs: string[], draft: BayesModelDraftDTO): string[] {
  const labels = distributionArgLabels(distribution);
  const fallbackArgs = initialDistributionArgs({ ...draft, likelihood: likelihoodFromDistribution(distribution) });
  return labels.map((_, index) => currentArgs[index] ?? fallbackArgs[index] ?? '');
}

function distributionArgLabels(distribution: LikelihoodDistribution): string[] {
  switch (distribution) {
    case 'normal':
      return ['均值 / predictor', '标准差 / sigma'];
    case 'bernoulli_logit':
      return ['logit'];
    case 'poisson_log':
      return ['log rate'];
  }
}

export function composeLikelihoodLatex(responseExpression: string, distribution: LikelihoodDistribution, args: string[]): string {
  const response = responseExpression.trim() || 'y';
  const safeArgs = distributionArgLabels(distribution).map((_, index) => args[index]?.trim() || '\\cdots');
  return `${response} \\sim \\operatorname{${distributionLatexName(distribution)}}\\left(${safeArgs.join(', ')}\\right)`;
}

function likelihoodFromFormulaParts(
  distribution: LikelihoodDistribution,
  args: string[],
  current: LikelihoodSpecDTO,
): LikelihoodSpecDTO {
  switch (distribution) {
    case 'normal':
      return {
        type: 'normal',
        mean: { source: 'predictor' },
        sigma: { parameter: latexToPlainSymbol(args[1]) || (current.type === 'normal' ? current.sigma.parameter : 'sigma') },
      };
    case 'bernoulli_logit':
      return { type: 'bernoulli_logit', logit: { source: 'predictor' } };
    case 'poisson_log':
      return { type: 'poisson_log', logRate: { source: 'predictor' } };
  }
}

function likelihoodFromDistribution(distribution: LikelihoodDistribution): LikelihoodSpecDTO {
  switch (distribution) {
    case 'normal':
      return { type: 'normal', mean: { source: 'predictor' }, sigma: { parameter: 'sigma' } };
    case 'bernoulli_logit':
      return { type: 'bernoulli_logit', logit: { source: 'predictor' } };
    case 'poisson_log':
      return { type: 'poisson_log', logRate: { source: 'predictor' } };
  }
}



function distributionLatexName(distribution: LikelihoodDistribution): string {
  switch (distribution) {
    case 'normal':
      return 'Normal';
    case 'bernoulli_logit':
      return 'BernoulliLogit';
    case 'poisson_log':
      return 'PoissonLog';
  }
}

function extractPredictorLatex(formulaText: string): string | null {
  const trimmed = formulaText.trim();
  const equalsIndex = trimmed.indexOf('=');
  if (equalsIndex >= 0) return trimmed.slice(equalsIndex + 1).trim() || null;
  const normalMatch = trimmed.match(/\\operatorname\{(?:Normal|BernoulliLogit|PoissonLog)\}\\left\((.*)\\right\)$/);
  if (normalMatch?.[1]) return normalMatch[1].split(',')[0]?.trim() || null;
  return null;
}

const LATEX_GREEK_SYMBOLS = new Set([
  'alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'eta', 'theta', 'iota', 'kappa',
  'lambda', 'mu', 'nu', 'xi', 'pi', 'rho', 'sigma', 'tau', 'upsilon', 'phi',
  'chi', 'psi', 'omega',
]);

export function latexSymbol(value: string): string {
  if (LATEX_GREEK_SYMBOLS.has(value)) return `\\${value}`;
  const indexed = value.match(/^([A-Za-z]+)_(?:\{([A-Za-z0-9_]+)\}|([A-Za-z0-9_]+))$/);
  if (!indexed) return value;
  const [, base, bracedIndex, plainIndex] = indexed;
  const renderedBase = LATEX_GREEK_SYMBOLS.has(base) ? `\\${base}` : base;
  return `${renderedBase}_{${bracedIndex ?? plainIndex}}`;
}

function latexToPlainSymbol(value: string | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  if (trimmed === '\\sigma') return 'sigma';
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed) ? trimmed : null;
}

function replaceAt(values: string[], index: number, value: string): string[] {
  const next = [...values];
  next[index] = value;
  return next;
}

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
      <CardHeader><PanelTitle title="2. Symbols" issues={issues} /></CardHeader>
      <CardContent className="space-y-3">
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>Symbol</TableHead><TableHead>Role</TableHead><TableHead>Data</TableHead><TableHead>Column</TableHead><TableHead>Prior</TableHead><TableHead>Bounds</TableHead><TableHead className="w-32">Actions</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {symbolsInDisplayOrder(draft.symbols).map(symbol => (
                <TableRow key={symbol.name}>
                  <TableCell><LatexInline formulaText={latexSymbol(symbol.name)} /></TableCell>
                  <TableCell>{roleLabel(symbol.role)}</TableCell>
                  <TableCell>{dataSourceLabel(draft, symbol.role, datasets)}</TableCell>
                  <TableCell className="font-mono">{columnLabel(draft, symbol.name, symbol.role)}</TableCell>
                  <TableCell className="font-mono">{priorLabel(draft, symbol.name, symbol.role)}</TableCell>
                  <TableCell className="font-mono">{boundsLabel(draft, symbol.name)}</TableCell>
                  <TableCell>
                    <Button size="sm" variant="outline" onClick={() => beginEdit(symbol.name)}>编辑</Button>
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
          onSave={() => {
            if (!editingSymbol) return;
            saveSymbolChanges(editingSymbol);
          }}
        />
      </CardContent>
    </Card>
  );
}

function SymbolRoleSelect({ value, onChange }: { value: BayesSymbolRoleDTO; onChange: (role: BayesSymbolRoleDTO) => void }) {
  return (
    <Select value={value} onValueChange={(nextValue) => onChange(nextValue as BayesSymbolRoleDTO)}>
      <SelectTrigger className="w-40 max-w-full"><SelectValue /></SelectTrigger>
      <SelectContent>
        <SelectItem value="dependent">因变量</SelectItem>
        <SelectItem value="independent">自变量</SelectItem>
        <SelectItem value="parameter">参数</SelectItem>
      </SelectContent>
    </Select>
  );
}



function numericColumns(columns: BayesDatasetSelectionDTO['columns']): BayesDatasetSelectionDTO['columns'] {
  return columns.filter(column => column.dtype === 'number' || column.dtype === 'integer');
}

function preferredSymbolColumn(dataset: BayesDatasetSelectionDTO, symbolName: string): string | null {
  const columns = numericColumns(dataset.columns);
  return columns.find(column => column.name === symbolName)?.name
    ?? columns[0]?.name
    ?? null;
}



function SymbolDetailEditor({
  columns,
  value,
  onValueChange,
}: {
  columns: BayesDatasetSelectionDTO['columns'];
  value: string;
  onValueChange: (value: string) => void;
}) {
  return (
    <Select value={value} onValueChange={onValueChange} disabled={columns.length === 0}>
      <SelectTrigger><SelectValue placeholder="选择数据列" /></SelectTrigger>
      <SelectContent>
        {columns.map(column => (
          <SelectItem key={column.name} value={column.name}>{column.name} · {column.dtype}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function SymbolConfigDialog({
  open,
  datasets,
  symbol,
  selectedDatasetId,
  role,
  detailValue,
  constraint,
  priorDistribution,
  priorArgs,
  onDatasetChange,
  onRoleChange,
  onDetailValueChange,
  onConstraintChange,
  onPriorDistributionChange,
  onPriorArgsChange,
  onClose,
  onSave,
}: {
  open: boolean;
  datasets: BayesDatasetOption[];
  symbol: string | null;
  selectedDatasetId: string;
  role: BayesSymbolRoleDTO;
  detailValue: string;
  constraint: ParameterConstraintDTO;
  priorDistribution: PriorSpecDTO['distribution'];
  priorArgs: string[];
  onDatasetChange: (sourceId: string) => void;
  onRoleChange: (role: BayesSymbolRoleDTO) => void;
  onDetailValueChange: (value: string) => void;
  onConstraintChange: (constraint: ParameterConstraintDTO) => void;
  onPriorDistributionChange: (distribution: PriorSpecDTO['distribution']) => void;
  onPriorArgsChange: (args: string[]) => void;
  onClose: () => void;
  onSave: () => void;
}) {
  const selectedDataset = datasets.find(dataset => dataset.sourceId === selectedDatasetId) ?? null;
  const selectedColumns = numericColumns(selectedDataset?.columns ?? []);
  const priorLabels = priorArgLabels(priorDistribution);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent
        explicitClose
        className="grid max-h-[85vh] w-[min(calc(100vw-2rem),44rem)] max-w-none grid-rows-[auto_minmax(0,1fr)_auto] rounded-lg"
      >
        <DialogHeader className="flex flex-row items-center justify-between gap-3 border-b border-border bg-muted/30">
          <DialogTitle>Symbol configuration</DialogTitle>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label="关闭"
            className="ml-auto"
            onClick={onClose}
          >
            <span aria-hidden="true" className="text-lg leading-none">×</span>
          </Button>
        </DialogHeader>
        <div className="min-h-0 space-y-4 overflow-y-auto px-6 py-5">
          <section className="grid gap-4 rounded-md border border-border bg-muted/10 p-4 md:grid-cols-[minmax(8rem,1fr)_minmax(0,2fr)]">
            <div className="flex min-w-0 items-center gap-3">
              <Label className="w-14 shrink-0 text-xs font-medium text-muted-foreground">Symbol</Label>
              <div className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                <LatexInline formulaText={latexSymbol(symbol ?? '')} />
              </div>
            </div>
            <div className="flex min-w-0 items-center gap-3">
              <Label className="w-10 shrink-0 text-xs font-medium text-muted-foreground">Role</Label>
              <div className="min-w-0 flex-1">
                <SymbolRoleSelect value={role} onChange={onRoleChange} />
              </div>
            </div>
          </section>

          {role === 'parameter' ? (
            <div className="space-y-4">
              <section className="space-y-3 rounded-md border border-border p-4">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">Parameter constraint</h3>
                  <span className="text-xs text-muted-foreground">
                    <LatexInline formulaText={`${latexSymbol(symbol ?? 'parameter')} \\in ${constraintSetLatex(constraint)}`} />
                  </span>
                </div>
                <div className="flex w-full items-center gap-3">
                  <Label className="w-18 shrink-0 text-xs text-muted-foreground">Constraint</Label>
                                    <div className="w-48 max-w-full">
                    <ConstraintSelect value={constraint.type} onChange={(type) => onConstraintChange(defaultConstraint(type, constraint))} />
                  </div>
                </div>
                <BoundsEditor constraint={constraint} onChange={onConstraintChange} />
              </section>
              <section className="space-y-3 rounded-md border border-border p-4">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">Prior</h3>
                  <span className="text-xs text-muted-foreground">
                    <LatexInline formulaText={priorSummaryLatex(symbol, priorDistribution, priorArgs)} />
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <Label className="w-24 shrink-0 text-xs text-muted-foreground">Prior distribution</Label>
                                    <div className="w-64 max-w-full">
                    <Select value={priorDistribution} onValueChange={(value) => onPriorDistributionChange(value as PriorSpecDTO['distribution'])}>
                      <SelectTrigger><SelectValue placeholder="选择分布" /></SelectTrigger>
                      <SelectContent>
                        {priorDistributionsForConstraint(constraint).map(distribution => (
                          <SelectItem key={distribution} value={distribution}>{priorDistributionLabel(distribution)}{isPriorCompatibleWithConstraint(distribution, constraint) ? ' · recommended' : ''}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div className={`grid gap-3 ${priorParameterGridClass(priorLabels.length)}`}>
                  {priorLabels.map((label, index) => (
                    <div key={label} className="space-y-1.5">
                      <Label className="text-xs text-muted-foreground">{label}</Label>
                      <Input
                        aria-label={label}
                        value={priorArgs[index] ?? ''}
                        className="font-mono"
                        onChange={(event) => onPriorArgsChange(replaceAt(priorArgs, index, event.target.value))}
                      />
                    </div>
                  ))}
                </div>
              </section>
            </div>
          ) : (
            <section className="space-y-4 rounded-md border border-border p-4">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">Data binding</h3>
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">Data source</Label>
                <Select value={selectedDatasetId} onValueChange={(sourceId) => {
                  onDatasetChange(sourceId);
                  const dataset = datasets.find(item => item.sourceId === sourceId);
                  const nextColumn = dataset ? preferredSymbolColumn(dataset, symbol ?? '') : null;
                  onDetailValueChange(nextColumn ?? '');
                }}>
                  <SelectTrigger><SelectValue placeholder="选择数据源" /></SelectTrigger>
                  <SelectContent>
                    {datasets.map(dataset => (
                      <SelectItem key={dataset.sourceId} value={dataset.sourceId}>{dataset.displayName}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                  {datasets.length === 0 ? <p className="text-xs text-muted-foreground">当前项目没有可用数据源，请先导入数据。</p> : null}
                </div>
                <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">Data column</Label>
                <SymbolDetailEditor columns={selectedColumns} value={detailValue} onValueChange={onDetailValueChange} />
                  {selectedDataset && selectedColumns.length === 0 ? <p className="text-xs text-muted-foreground">当前数据源没有列信息，正在同步或请刷新数据源。</p> : null}
                </div>
              </div>
            </section>
          )}
        </div>
        <DialogFooter className="shrink-0">
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button onClick={onSave}>保存</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const PRIOR_DISTRIBUTIONS: PriorSpecDTO['distribution'][] = [
  'normal',
  'log_normal',
  'uniform',
  'beta',
  'gamma',
  'exponential',
  'student_t',
  'cauchy',
  'half_normal',
];

function priorDistributionsForConstraint(constraint: ParameterConstraintDTO): PriorSpecDTO['distribution'][] {
  const recommended = PRIOR_DISTRIBUTIONS.filter(distribution => isPriorCompatibleWithConstraint(distribution, constraint));
  const others = PRIOR_DISTRIBUTIONS.filter(distribution => !recommended.includes(distribution));
  return [...recommended, ...others];
}

function isPriorCompatibleWithConstraint(distribution: PriorSpecDTO['distribution'], constraint: ParameterConstraintDTO): boolean {
  switch (constraint.type) {
    case 'real':
      return ['normal', 'student_t', 'cauchy', 'uniform'].includes(distribution);
    case 'positive':
      return ['log_normal', 'gamma', 'exponential', 'half_normal'].includes(distribution);
    case 'unit':
      return ['beta', 'uniform'].includes(distribution);
    case 'bounded':
      return distribution === 'uniform';
  }
}



function ConstraintSelect({ value, onChange }: { value: ParameterConstraintDTO['type']; onChange: (type: ParameterConstraintDTO['type']) => void }) {
  return (
    <Select value={value} onValueChange={(nextValue) => onChange(nextValue as ParameterConstraintDTO['type'])}>
      <SelectTrigger><SelectValue /></SelectTrigger>
      <SelectContent>
        <SelectItem value="real">real</SelectItem>
        <SelectItem value="positive">positive</SelectItem>
        <SelectItem value="unit">unit</SelectItem>
        <SelectItem value="bounded">bounded</SelectItem>
      </SelectContent>
    </Select>
  );
}

function BoundsEditor({ constraint, onChange }: { constraint: ParameterConstraintDTO; onChange: (constraint: ParameterConstraintDTO) => void }) {
  if (constraint.type !== 'bounded') return null;

  return (
    <div className="grid gap-3 md:grid-cols-4">
      <div className="space-y-1.5">
        <Label className="text-xs text-muted-foreground">Lower bound</Label>
        <Input
          type="number"
          value={constraint.lower}
          className="font-mono"
          onChange={(event) => onChange({ ...constraint, lower: Number(event.target.value) })}
        />
      </div>
      <div className="space-y-1.5">
        <Label className="text-xs text-muted-foreground">Upper bound</Label>
        <Input
          type="number"
          value={constraint.upper}
          className="font-mono"
          onChange={(event) => onChange({ ...constraint, upper: Number(event.target.value) })}
        />
      </div>
      <BooleanSelect label="Include lower" value={constraint.includeLower} onChange={(includeLower) => onChange({ ...constraint, includeLower })} />
      <BooleanSelect label="Include upper" value={constraint.includeUpper} onChange={(includeUpper) => onChange({ ...constraint, includeUpper })} />
    </div>
  );
}

function BooleanSelect({ label, value, onChange }: { label: string; value: boolean; onChange: (value: boolean) => void }) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Select value={String(value)} onValueChange={(nextValue) => onChange(nextValue === 'true')}>
        <SelectTrigger><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="true">true</SelectItem>
          <SelectItem value="false">false</SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}

function defaultConstraint(type: ParameterConstraintDTO['type'], previous: ParameterConstraintDTO): ParameterConstraintDTO {
  switch (type) {
    case 'real':
      return { type: 'real' };
    case 'positive':
      return { type: 'positive' };
    case 'unit':
      return { type: 'unit' };
    case 'bounded':
      return previous.type === 'bounded'
        ? previous
        : { type: 'bounded', lower: 0, upper: 1, includeLower: false, includeUpper: false };
  }
}

function symbolsInDisplayOrder(symbols: readonly BayesModelDraftDTO['symbols'][number][]): BayesModelDraftDTO['symbols'] {
  const roleOrder: Record<BayesSymbolRoleDTO, number> = { dependent: 0, independent: 1, parameter: 2 };
  return symbols
    .map((symbol, index) => ({ symbol, index }))
    .sort((left, right) => roleOrder[left.symbol.role] - roleOrder[right.symbol.role] || left.index - right.index)
    .map(({ symbol }) => symbol);
}

function roleLabel(role: BayesSymbolRoleDTO): string {
  switch (role) {
    case 'dependent':
      return '因变量';
    case 'independent':
      return '自变量';
    case 'parameter':
      return '参数';
  }
}

function parameterForSymbol(draft: BayesModelDraftDTO, name: string) {
  return draft.parameters.find(parameter => parameter.name === name);
}



function boundsLabel(draft: BayesModelDraftDTO, name: string): string {
  const constraint = parameterForSymbol(draft, name)?.constraint;
  return constraint ? boundsSummary(constraint) : '—';
}

function constraintSetLatex(constraint: ParameterConstraintDTO): string {
  switch (constraint.type) {
    case 'real':
      return '(-\\infty, \\infty)';
    case 'positive':
      return '(0, \\infty)';
    case 'unit':
      return '(0, 1)';
    case 'bounded': {
      const left = constraint.includeLower ? '[' : '(';
      const right = constraint.includeUpper ? ']' : ')';
      return `${left}${constraint.lower}, ${constraint.upper}${right}`;
    }
  }
}

function priorSummaryLatex(
  symbol: string | null,
  distribution: PriorSpecDTO['distribution'],
  args: readonly string[],
): string {
  const distributionName = priorDistributionLabel(distribution);
  const values = args.slice(0, priorArgLabels(distribution).length).map(value => value || '\\cdots');
  return `${latexSymbol(symbol ?? 'parameter')} \\sim \\operatorname{${distributionName}}\\left(${values.join(', ')}\\right)`;
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
): string {
  if (role === 'parameter') return '—';
  if (!draft.dataset) return '未选择数据';
  return datasets.find(dataset => dataset.sourceId === draft.dataset?.sourceId)?.displayName ?? '未知数据源';
}

function columnLabel(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role === 'parameter') return '—';
  return symbolDetailValue(draft, name, role) || '未绑定列';
}

function priorLabel(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role !== 'parameter') return '—';
  const parameter = parameterForSymbol(draft, name);
  return parameter ? formatPrior(parameter.prior) : '未设置分布';
}

function priorDistributionLabel(distribution: PriorSpecDTO['distribution']): string {
  return distribution
    .split('_')
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

function datasetSelectionFromOption(option: BayesDatasetOption): BayesDatasetSelectionDTO {
  return {
    sourceType: option.sourceType,
    sourceId: option.sourceId,
    columns: option.columns,
  };
}

function priorParameterGridClass(parameterCount: number): string {
  if (parameterCount === 1) return 'md:grid-cols-1';
  if (parameterCount === 2) return 'md:grid-cols-2';
  return 'md:grid-cols-3';
}

function priorArgLabels(distribution: PriorSpecDTO['distribution']): string[] {
  switch (distribution) {
    case 'normal':
      return ['Mean', 'Standard deviation'];
    case 'log_normal':
      return ['Log mean', 'Log standard deviation'];
    case 'uniform':
      return ['Lower bound', 'Upper bound'];
    case 'beta':
      return ['Alpha', 'Beta'];
    case 'gamma':
      return ['Shape', 'Scale'];
    case 'exponential':
      return ['Scale'];
    case 'student_t':
      return ['Degrees of freedom', 'Location', 'Scale'];
    case 'cauchy':
      return ['Location', 'Scale'];
    case 'half_normal':
      return ['Scale'];
  }
}

function defaultPriorArgs(distribution: PriorSpecDTO['distribution']): number[] {
  switch (distribution) {
    case 'normal':
      return [0, 10];
    case 'log_normal':
      return [0, 1];
    case 'uniform':
      return [0, 1];
    case 'beta':
      return [2, 2];
    case 'gamma':
      return [2, 1];
    case 'exponential':
      return [1];
    case 'student_t':
      return [3, 0, 10];
    case 'cauchy':
      return [0, 2.5];
    case 'half_normal':
      return [5];
  }
}

function priorFromParts(distribution: PriorSpecDTO['distribution'], args: string[]): PriorSpecDTO {
  const values = priorArgLabels(distribution).map((_, index) => Number(args[index]));
  const fallback = defaultPriorArgs(distribution);
  const safe = values.map((value, index) => Number.isFinite(value) ? value : fallback[index]);
  switch (distribution) {
    case 'normal':
    case 'log_normal':
    case 'uniform':
    case 'beta':
    case 'gamma':
    case 'cauchy':
      return { distribution, args: [safe[0] ?? fallback[0], safe[1] ?? fallback[1]] } as PriorSpecDTO;
    case 'student_t':
      return { distribution, args: [safe[0] ?? 3, safe[1] ?? 0, safe[2] ?? 10] };
    case 'exponential':
    case 'half_normal':
      return { distribution, args: [safe[0] ?? fallback[0]] } as PriorSpecDTO;
  }
}



export function SamplerStep({ draft, onSamplerChange }: { draft: BayesModelDraftDTO; onSamplerChange: (sampler: InferenceConfigDTO) => void }) {
  const updateNumber = (key: keyof InferenceConfigDTO, value: string) => {
    const numberValue = Number(value);
    if (!Number.isFinite(numberValue)) return;
    onSamplerChange({ ...draft.sampler, [key]: numberValue });
  };

  return (
    <Card>
      <CardHeader><PanelTitle title="3. Sampler" description="第一版普通用户只暴露 NUTS" /></CardHeader>
      <CardContent className="grid grid-cols-2 gap-3 lg:grid-cols-3">
        <ReadOnlyField label="Algorithm" value={draft.sampler.algorithm.toUpperCase()} />
        <EditableNumberField label="Chains" value={draft.sampler.chains} min={1} onChange={(value) => updateNumber('chains', value)} />
        <EditableNumberField label="Samples" value={draft.sampler.samples} min={1} onChange={(value) => updateNumber('samples', value)} />
        <EditableNumberField label="Warmup" value={draft.sampler.warmup} min={0} onChange={(value) => updateNumber('warmup', value)} />
        <EditableNumberField label="Seed" value={draft.sampler.seed ?? 1234} min={0} onChange={(value) => updateNumber('seed', value)} />
        <EditableNumberField label="Target accept" value={draft.sampler.targetAccept ?? 0.8} min={0} max={1} step={0.01} onChange={(value) => updateNumber('targetAccept', value)} />
        <EditableNumberField label="Max tree depth" value={draft.sampler.maxTreeDepth ?? 10} min={1} onChange={(value) => updateNumber('maxTreeDepth', value)} />
      </CardContent>
    </Card>
  );
}






export function PanelTitle({
  title,
  description,
  issues = [],
}: {
  title: string;
  description?: string;
  issues?: ValidationIssueDTO[];
}) {
  return (
    <div className="space-y-1">
      <h2 className="text-sm font-semibold text-foreground">{title}</h2>
      {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
      {issues.map(issue => (
        <p key={`${issue.code}-${issue.path ?? ''}`} className={issue.severity === 'error' ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}>
          <span className="font-mono">[{issue.code}]</span> {issue.message}
        </p>
      ))}
    </div>
  );
}

function formulaErrorIssue(error: FormulaParseError): ValidationIssueDTO {
  return {
    code: error.code,
    severity: 'error',
    message: `${error.message}${error.detail ? ` (${error.detail})` : ''}`,
    path: 'formulaText',
  };
}

function EditableNumberField({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
}: {
  label: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        className="h-9"
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function ReadOnlyField({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <div className={`rounded-md border border-border bg-muted/30 px-3 py-2 text-sm ${mono ? 'font-mono' : ''}`}>{value}</div>
    </div>
  );
}



function RecognizedSymbols({ symbols }: { symbols: string[] }) {
  const uniqueSymbols = Array.from(new Set(symbols)).sort();
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
      <span>识别的 symbols:</span>
      {uniqueSymbols.length > 0
        ? uniqueSymbols.map(symbol => (
          <span key={symbol} className="rounded border border-border bg-muted/30 px-1.5 py-0.5 text-foreground">
            <LatexInline formulaText={latexSymbol(symbol)} />
          </span>
        ))
        : <span>无</span>}
    </div>
  );
}

function LatexFormulaPreview({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderLatex(formulaText, true), [formulaText]);
  return (
    <div
      className="rounded-md border border-border bg-muted/30 px-3 py-3 text-sm overflow-x-auto [&_.katex]:text-foreground [&_.katex-display]:my-0"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

export function LatexInline({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderLatex(formulaText, false), [formulaText]);
  return <span className="[&_.katex]:text-foreground" dangerouslySetInnerHTML={{ __html: html }} />;
}

function renderLatex(formulaText: string, displayMode: boolean): string {
  const latex = formulaText.trim() || '\\cdots';
  try {
    return katex.renderToString(latex, {
      displayMode,
      throwOnError: false,
    });
  } catch {
    return escapeHtml(latex);
  }
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => {
    switch (character) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return character;
    }
  });
}

export function formatNumber(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : '—';
}

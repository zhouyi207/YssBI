
import { useEffect, useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { BayesColumnDTypeDTO, BayesDatasetSelectionDTO, BayesModelDraftDTO, BayesSymbolRoleDTO, InferenceConfigDTO, LikelihoodSpecDTO, ParameterConstraintDTO, PriorSpecDTO } from '@/shared/types/bayes';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { defaultPriorForConstraint, formatExpression, formatPrior } from '@/features/domain/bayes';


export function FormulaStep({
  draft,
  onModelEquationChange,
}: {
  draft: BayesModelDraftDTO;
  onModelEquationChange: (responseSymbol: string, formulaText: string, likelihood: LikelihoodSpecDTO, predictorText?: string) => void | Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [responseSymbol, setResponseSymbol] = useState(currentResponseSymbol(draft));
  const [distribution, setDistribution] = useState<LikelihoodDistribution>(likelihoodDistribution(draft.likelihood));
  const [distributionArgs, setDistributionArgs] = useState<string[]>(() => initialDistributionArgs(draft));

  useEffect(() => {
    if (!editing) {
      setResponseSymbol(currentResponseSymbol(draft));
      setDistribution(likelihoodDistribution(draft.likelihood));
      setDistributionArgs(initialDistributionArgs(draft));
    }
  }, [draft, editing]);

  const applyDistribution = (nextDistribution: LikelihoodDistribution) => {
    setDistribution(nextDistribution);
    setDistributionArgs(currentArgs => resizeDistributionArgs(nextDistribution, currentArgs, draft));
  };

  const commit = async () => {
    const nextResponse = responseSymbol.trim() || 'y';
    const nextFormulaText = composeLikelihoodLatex(nextResponse, distribution, distributionArgs);
    await onModelEquationChange(nextResponse, nextFormulaText, likelihoodFromFormulaParts(distribution, distributionArgs, draft.likelihood), distributionArgs[0]);
    setEditing(false);
  };

  const cancel = () => {
    setResponseSymbol(currentResponseSymbol(draft));
    setDistribution(likelihoodDistribution(draft.likelihood));
    setDistributionArgs(initialDistributionArgs(draft));
    setEditing(false);
  };

  return (
    <Card>
      <CardHeader><PanelTitle title="1. Formula" description="先输入数学模型，再决定符号含义" /></CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1.5">
          <div className="flex items-center justify-between gap-3">
            <Label htmlFor="bayes-response-symbol">Model equation</Label>
            <Button size="sm" variant="outline" onClick={() => setEditing(true)} disabled={editing}>
              编辑
            </Button>
          </div>
          {editing ? (
            <div className="space-y-3 rounded-md border border-border bg-muted/20 p-3">
              <div className="grid gap-3 md:grid-cols-[120px_180px_minmax(0,1fr)]">
                <div className="space-y-1.5">
                  <Label htmlFor="bayes-response-symbol" className="text-xs text-muted-foreground">因变量</Label>
                  <Input
                    id="bayes-response-symbol"
                    value={responseSymbol}
                    autoFocus
                    className="h-8 font-mono"
                    onChange={(event) => setResponseSymbol(event.target.value)}
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
              <LatexFormulaPreview formulaText={composeLikelihoodLatex(responseSymbol || 'y', distribution, distributionArgs)} />
              <p className="text-xs text-muted-foreground">{likelihoodInputHint(distribution)}</p>
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="outline" onClick={cancel}>取消</Button>
                <Button size="sm" onClick={commit}>保存</Button>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              <LatexFormulaPreview formulaText={draft.formulaText} />
              <p className="text-xs text-muted-foreground">{likelihoodInputHint(likelihoodDistribution(draft.likelihood))}</p>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

type LikelihoodDistribution = LikelihoodSpecDTO['type'];

function currentResponseSymbol(draft: BayesModelDraftDTO): string {
  return draft.responseSymbol ?? draft.responseBinding?.symbol ?? 'y';
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

function composeLikelihoodLatex(responseSymbol: string, distribution: LikelihoodDistribution, args: string[]): string {
  const response = responseSymbol.trim() || 'y';
  const safeArgs = distributionArgLabels(distribution).map((_, index) => args[index]?.trim() || '\\cdots');
  return `${latexSymbol(response)} \\sim \\operatorname{${distributionLatexName(distribution)}}\\left(${safeArgs.join(', ')}\\right)`;
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

function likelihoodInputHint(distribution: LikelihoodDistribution): string {
  switch (distribution) {
    case 'normal':
      return 'Normal 用于连续数值响应；需要 sigma 参数，sigma 应使用 positive 约束。';
    case 'bernoulli_logit':
      return 'BernoulliLogit 用于二分类响应；响应列必须是 boolean 或 0/1，不需要 sigma。';
    case 'poisson_log':
      return 'PoissonLog 用于计数响应；响应列必须是非负整数，不需要 sigma。';
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

function latexSymbol(value: string): string {
  if (value === 'sigma') return '\\sigma';
  return value;
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
  onDeleteSymbol,
}: {
  draft: BayesModelDraftDTO;
  datasets: BayesDatasetSelectionDTO[];
  onSymbolConfigurationChange: (configuration: {
    name: string;
    dataset: BayesDatasetSelectionDTO | null;
    role: BayesSymbolRoleDTO;
    column: string;
    constraint: ParameterConstraintDTO;
    prior: PriorSpecDTO;
  }) => void;
  onDeleteSymbol: (name: string) => void;
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
      dataset,
      role,
      column: detailValue,
      constraint,
      prior: priorFromParts(priorDistribution, priorArgs),
    });
    cancelEdit();
  };

  return (
    <Card>
      <CardHeader><PanelTitle title="2. Symbols" description="维护方程符号、角色以及对应数据或先验分布" /></CardHeader>
      <CardContent className="space-y-3">
        <p className="text-xs text-muted-foreground">{responseColumnHint(draft.likelihood)}</p>
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>Symbol</TableHead><TableHead>Role</TableHead><TableHead>Data / Prior</TableHead><TableHead>Constraint</TableHead><TableHead>Bounds</TableHead><TableHead className="w-32">Actions</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {draft.symbols.map(symbol => (
                <TableRow key={symbol.name}>
                  <TableCell><LatexInline formulaText={latexSymbol(symbol.name)} /></TableCell>
                  <TableCell>{roleLabel(symbol.role)}</TableCell>
                  <TableCell><span className="font-mono">{symbolDetailLabel(draft, symbol.name, symbol.role)}</span></TableCell>
                  <TableCell className="font-mono">{constraintLabel(draft, symbol.name)}</TableCell>
                  <TableCell className="font-mono">{boundsLabel(draft, symbol.name)}</TableCell>
                  <TableCell>
                    <div className="flex gap-1">
                      <Button size="sm" variant="outline" onClick={() => beginEdit(symbol.name)}>编辑</Button>
                      <Button size="sm" variant="ghost" onClick={() => onDeleteSymbol(symbol.name)}>删除</Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <SymbolConfigDialog
          open={editingSymbol !== null}
          draft={draft}
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
      <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
      <SelectContent>
        <SelectItem value="dependent">因变量</SelectItem>
        <SelectItem value="independent">自变量</SelectItem>
        <SelectItem value="parameter">参数</SelectItem>
      </SelectContent>
    </Select>
  );
}

function responseColumnHint(likelihood: LikelihoodSpecDTO): string {
  switch (likelihood.type) {
    case 'normal':
      return '响应列要求：连续数值列；自变量列要求：有限数值。';
    case 'bernoulli_logit':
      return '响应列要求：boolean 或只包含 0/1 的数值列；自变量列要求：有限数值。';
    case 'poisson_log':
      return '响应列要求：非负整数计数列；自变量列要求：有限数值。';
  }
}

function preferredSymbolColumn(
  dataset: BayesDatasetSelectionDTO,
  symbolName: string,
  role: BayesSymbolRoleDTO,
  likelihood: LikelihoodSpecDTO,
): string | null {
  const compatible = dataset.columns.filter(column => !columnCompatibilityHint(column.dtype, role, likelihood));
  return compatible.find(column => column.name === symbolName)?.name
    ?? compatible[0]?.name
    ?? dataset.columns[0]?.name
    ?? null;
}

function columnCompatibilityHint(dtype: BayesColumnDTypeDTO, role: BayesSymbolRoleDTO, likelihood: LikelihoodSpecDTO): string {
  if (role === 'independent') {
    return dtype === 'number' || dtype === 'integer' ? '' : ' · check type';
  }
  if (role !== 'dependent') return '';
  if (likelihood.type === 'normal') return dtype === 'number' || dtype === 'integer' ? '' : ' · check type';
  if (likelihood.type === 'bernoulli_logit') return dtype === 'boolean' || dtype === 'integer' || dtype === 'number' ? '' : ' · check type';
  return dtype === 'integer' || dtype === 'number' ? '' : ' · check type';
}



function SymbolDetailEditor({
  columns,
  role,
  value,
  likelihood,
  onValueChange,
}: {
  columns: BayesDatasetSelectionDTO['columns'];
  role: BayesSymbolRoleDTO;
  value: string;
  likelihood: LikelihoodSpecDTO;
  onValueChange: (value: string) => void;
}) {
  return (
    <Select value={value} onValueChange={onValueChange} disabled={columns.length === 0}>
      <SelectTrigger size="sm" className="w-48"><SelectValue placeholder="选择数据列" /></SelectTrigger>
      <SelectContent>
        {columns.map(column => (
          <SelectItem key={column.name} value={column.name}>{column.name} · {column.dtype}{columnCompatibilityHint(column.dtype, role, likelihood)}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function SymbolConfigDialog({
  open,
  draft,
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
  draft: BayesModelDraftDTO;
  datasets: BayesDatasetSelectionDTO[];
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
  const selectedColumns = selectedDataset?.columns ?? [];

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent className="max-w-[680px]">
        <DialogHeader className="border-b border-border bg-muted/20">
          <DialogTitle>Symbol configuration{symbol ? ` · ${symbol}` : ''}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 px-6 py-5">
          <div className="grid gap-3 md:grid-cols-2">
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">Symbol</Label>
              <div className="rounded-md border border-border bg-muted/30 px-3 py-2 text-sm">
                <LatexInline formulaText={latexSymbol(symbol ?? '')} />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">Role</Label>
              <SymbolRoleSelect value={role} onChange={onRoleChange} />
            </div>
          </div>

          {role === 'parameter' ? (
            <>
              <div className="grid gap-3 md:grid-cols-2">
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">Constraint</Label>
                  <ConstraintSelect value={constraint.type} onChange={(type) => onConstraintChange(defaultConstraint(type, constraint))} />
                </div>
              </div>
              <BoundsEditor constraint={constraint} onChange={onConstraintChange} />
              <div className="grid gap-3 md:grid-cols-[180px_minmax(0,1fr)]">
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">Prior distribution</Label>
                  <Select value={priorDistribution} onValueChange={(value) => onPriorDistributionChange(value as PriorSpecDTO['distribution'])}>
                    <SelectTrigger><SelectValue placeholder="选择分布" /></SelectTrigger>
                    <SelectContent>
                      {priorDistributionsForConstraint(constraint).map(distribution => (
                        <SelectItem key={distribution} value={distribution}>{priorDistributionLabel(distribution)}{isPriorCompatibleWithConstraint(distribution, constraint) ? ' · recommended' : ''}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">Prior args</Label>
                  <div className="grid gap-2 md:grid-cols-3">
                    {priorArgLabels(priorDistribution).map((label, index) => (
                      <Input
                        key={label}
                        aria-label={label}
                        value={priorArgs[index] ?? ''}
                        className="font-mono"
                        placeholder={label}
                        onChange={(event) => onPriorArgsChange(replaceAt(priorArgs, index, event.target.value))}
                      />
                    ))}
                  </div>
                </div>
              </div>
              <div className="space-y-1 rounded-md border border-border bg-muted/30 px-3 py-2 text-sm">
                <div className="font-mono">{symbol ?? 'parameter'} ~ {priorDistributionLabel(priorDistribution)}({priorArgs.filter(Boolean).join(', ')})；{constraintSummary(constraint)}</div>
                <p className="text-xs text-muted-foreground">{constraintPriorHint(constraint)}</p>
              </div>
            </>
          ) : (
            <div className="space-y-3">
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">Data source</Label>
                <Select value={selectedDatasetId} onValueChange={(sourceId) => {
                  onDatasetChange(sourceId);
                  const dataset = datasets.find(item => item.sourceId === sourceId);
                  const nextColumn = dataset ? preferredSymbolColumn(dataset, symbol ?? '', role, draft.likelihood) : null;
                  onDetailValueChange(nextColumn ?? '');
                }}>
                  <SelectTrigger><SelectValue placeholder="选择数据源" /></SelectTrigger>
                  <SelectContent>
                    {datasets.map(dataset => (
                      <SelectItem key={dataset.sourceId} value={dataset.sourceId}>{dataset.sourceId}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {datasets.length === 0 ? <p className="text-xs text-muted-foreground">当前项目没有可用数据源，请先导入数据。</p> : null}
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">Data column</Label>
                <SymbolDetailEditor columns={selectedColumns} role={role} value={detailValue} likelihood={draft.likelihood} onValueChange={onDetailValueChange} />
                {selectedDataset && selectedColumns.length === 0 ? <p className="text-xs text-muted-foreground">当前数据源没有列信息，正在同步或请刷新数据源。</p> : null}
                <p className="text-xs text-muted-foreground">{role === 'dependent' ? responseColumnHint(draft.likelihood) : '自变量列要求：有限数值。'}</p>
              </div>
            </div>
          )}
        </div>
        <DialogFooter>
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

function constraintPriorHint(constraint: ParameterConstraintDTO): string {
  switch (constraint.type) {
    case 'real':
      return 'real 参数推荐 Normal、StudentT、Cauchy 或合适范围的 Uniform。';
    case 'positive':
      return 'positive 参数推荐 LogNormal、Gamma、Exponential 或 HalfNormal。';
    case 'unit':
      return 'unit 参数推荐 Beta 或 Uniform(0, 1)。';
    case 'bounded':
      return 'bounded 参数推荐使用与上下界一致的 Uniform。';
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
  if (constraint.type !== 'bounded') {
    return <p className="text-xs text-muted-foreground">Bounds: {boundsSummary(constraint)}</p>;
  }

  return (
    <div className="grid gap-3 md:grid-cols-[1fr_1fr_150px_150px]">
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

function constraintLabel(draft: BayesModelDraftDTO, name: string): string {
  return parameterForSymbol(draft, name)?.constraint.type ?? '—';
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

function constraintSummary(constraint: ParameterConstraintDTO): string {
  return `${constraint.type} ${boundsSummary(constraint)}`;
}

function symbolDetailValue(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role === 'dependent') return draft.responseBinding?.symbol === name ? draft.responseBinding.column : '';
  if (role === 'independent') return draft.dataBindings[name] ?? '';
  return draft.parameters.find(parameter => parameter.name === name)?.prior.distribution ?? 'normal';
}

function defaultSymbolDetailValue(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  const current = symbolDetailValue(draft, name, role);
  if (current) return current;
  if (role === 'parameter') return 'normal';
  return draft.dataset?.columns[0]?.name ?? '';
}

function symbolDetailLabel(draft: BayesModelDraftDTO, name: string, role: BayesSymbolRoleDTO): string {
  if (role === 'parameter') {
    const parameter = parameterForSymbol(draft, name);
    return parameter ? `${parameter.constraint.type}; ${formatPrior(parameter.prior)}` : '未设置分布';
  }
  return symbolDetailValue(draft, name, role) || '未绑定数据';
}

function priorDistributionLabel(distribution: PriorSpecDTO['distribution']): string {
  return distribution
    .split('_')
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

function priorArgLabels(distribution: PriorSpecDTO['distribution']): string[] {
  switch (distribution) {
    case 'normal':
    case 'log_normal':
    case 'uniform':
    case 'beta':
    case 'gamma':
    case 'cauchy':
      return ['arg1', 'arg2'];
    case 'student_t':
      return ['df', 'loc', 'scale'];
    case 'exponential':
    case 'half_normal':
      return ['arg1'];
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






export function PanelTitle({ title, description }: { title: string; description: string }) {
  return (
    <div>
      <h2 className="text-sm font-semibold text-foreground">{title}</h2>
      <p className="text-xs text-muted-foreground">{description}</p>
    </div>
  );
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



function LatexFormulaPreview({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderLatex(formulaText, true), [formulaText]);
  return (
    <div
      className="rounded-md border border-border bg-muted/30 px-3 py-3 text-sm overflow-x-auto [&_.katex]:text-foreground [&_.katex-display]:my-0"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function LatexInline({ formulaText }: { formulaText: string }) {
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

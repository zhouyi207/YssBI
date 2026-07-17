import { useEffect, useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { BayesDatasetSelectionDTO, BayesModelDraftDTO, BayesSymbolRoleDTO, InferenceConfigDTO, InferenceResultDTO, LikelihoodSpecDTO, ParameterConstraintDTO, PriorSpecDTO } from '@/shared/types/bayes';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatExpression, formatPrior } from '@/features/domain/bayes';

export function FormulaStep({
  draft,
  onModelEquationChange,
}: {
  draft: BayesModelDraftDTO;
  onModelEquationChange: (responseSymbol: string, formulaText: string, likelihood: LikelihoodSpecDTO, predictorText?: string) => void;
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

  const commit = () => {
    const nextResponse = responseSymbol.trim() || 'y';
    const nextFormulaText = composeLikelihoodLatex(nextResponse, distribution, distributionArgs);
    onModelEquationChange(nextResponse, nextFormulaText, likelihoodFromFormulaParts(distribution, distributionArgs, draft.likelihood), distributionArgs[0]);
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
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="outline" onClick={cancel}>取消</Button>
                <Button size="sm" onClick={commit}>保存</Button>
              </div>
            </div>
          ) : (
            <LatexFormulaPreview formulaText={draft.formulaText} />
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
  onDatasetChange,
  onSymbolNameChange,
  onSymbolRoleChange,
  onSymbolDataBindingChange,
  onSymbolPriorChange,
  onSymbolConstraintChange,
  onDeleteSymbol,
}: {
  draft: BayesModelDraftDTO;
  datasets: BayesDatasetSelectionDTO[];
  onDatasetChange: (dataset: BayesDatasetSelectionDTO | null) => void;
  onSymbolNameChange: (oldName: string, newName: string) => void;
  onSymbolRoleChange: (name: string, role: BayesSymbolRoleDTO) => void;
  onSymbolDataBindingChange: (name: string, column: string) => void;
  onSymbolPriorChange: (name: string, prior: PriorSpecDTO) => void;
  onSymbolConstraintChange: (name: string, constraint: ParameterConstraintDTO) => void;
  onDeleteSymbol: (name: string) => void;
}) {
  const [editingSymbol, setEditingSymbol] = useState<string | null>(null);
  const [symbolName, setSymbolName] = useState('');
  const [role, setRole] = useState<BayesSymbolRoleDTO>('parameter');
  const [detailValue, setDetailValue] = useState('');
  const [constraint, setConstraint] = useState<ParameterConstraintDTO>({ type: 'real' });
  const [priorDistribution, setPriorDistribution] = useState<PriorSpecDTO['distribution']>('normal');
  const [priorArgs, setPriorArgs] = useState<string[]>(['0', '10']);

  const beginEdit = (symbolName: string) => {
    const symbol = draft.symbols.find(item => item.name === symbolName);
    if (!symbol) return;
    setEditingSymbol(symbol.name);
    setSymbolName(symbol.name);
    setRole(symbol.role);
    setDetailValue(symbolDetailValue(draft, symbol.name, symbol.role));
    const parameter = draft.parameters.find(item => item.name === symbol.name);
    setConstraint(parameter?.constraint ?? { type: 'real' });
    setPriorDistribution(parameter?.prior.distribution ?? 'normal');
    setPriorArgs((parameter?.prior.args ?? [0, 10]).map(String));
  };

  const cancelEdit = () => {
    setEditingSymbol(null);
    setSymbolName('');
    setRole('parameter');
    setDetailValue('');
  };

  const saveEdit = () => {
    if (!editingSymbol) return;
    const nextName = symbolName.trim();
    if (!nextName) return;
    if (nextName !== editingSymbol) onSymbolNameChange(editingSymbol, nextName);
    onSymbolRoleChange(nextName, role);
    if (role === 'parameter') {
      onSymbolConstraintChange(nextName, constraint);
      onSymbolPriorChange(nextName, priorFromParts(priorDistribution, priorArgs));
    } else if (detailValue) {
      onSymbolDataBindingChange(nextName, detailValue);
    }
    cancelEdit();
  };

  return (
    <Card>
      <CardHeader><PanelTitle title="2. Symbols" description="维护方程符号、角色以及对应数据或先验分布" /></CardHeader>
      <CardContent className="space-y-3">
        <DatasetSelect datasets={datasets} value={draft.dataset?.sourceId ?? ''} onChange={(sourceId) => onDatasetChange(datasets.find(dataset => dataset.sourceId === sourceId) ?? null)} />
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>Symbol</TableHead><TableHead>Role</TableHead><TableHead>Data / Prior</TableHead><TableHead className="w-32">Actions</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {draft.symbols.map(symbol => {
                const editing = editingSymbol === symbol.name;
                return (
                  <TableRow key={symbol.name}>
                    <TableCell className="font-mono">
                      {editing ? <Input value={symbolName} className="h-7 font-mono" onChange={event => setSymbolName(event.target.value)} /> : symbol.name}
                    </TableCell>
                    <TableCell>
                      {editing ? (
                        <SymbolRoleSelect value={role} onChange={(nextRole) => {
                          setRole(nextRole);
                          setDetailValue(defaultSymbolDetailValue(draft, symbol.name, nextRole));
                        }} />
                      ) : roleLabel(symbol.role)}
                    </TableCell>
                    <TableCell>
                      {editing
                        ? <SymbolDetailEditor
                          draft={draft}
                          role={role}
                          value={detailValue}
                          constraint={constraint}
                          priorDistribution={priorDistribution}
                          priorArgs={priorArgs}
                          onValueChange={setDetailValue}
                          onConstraintChange={setConstraint}
                          onPriorDistributionChange={(distribution) => {
                            setPriorDistribution(distribution);
                            setPriorArgs(defaultPriorArgs(distribution).map(String));
                          }}
                          onPriorArgsChange={setPriorArgs}
                        />
                        : <span className="font-mono">{symbolDetailLabel(draft, symbol.name, symbol.role)}</span>}
                    </TableCell>
                    <TableCell>
                      <div className="flex gap-1">
                        {editing ? (
                          <>
                            <Button size="sm" variant="default" onClick={saveEdit}>保存</Button>
                            <Button size="sm" variant="outline" onClick={cancelEdit}>取消</Button>
                          </>
                        ) : (
                          <>
                            <Button size="sm" variant="outline" onClick={() => beginEdit(symbol.name)}>编辑</Button>
                            <Button size="sm" variant="ghost" onClick={() => onDeleteSymbol(symbol.name)}>删除</Button>
                          </>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>

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

function DatasetSelect({ datasets, value, onChange }: { datasets: BayesDatasetSelectionDTO[]; value: string; onChange: (sourceId: string) => void }) {
  return (
    <div className="grid max-w-xl gap-1.5">
      <Label className="text-xs text-muted-foreground">Data source</Label>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger size="sm"><SelectValue placeholder="选择数据源" /></SelectTrigger>
        <SelectContent>
          {datasets.map(dataset => <SelectItem key={dataset.sourceId} value={dataset.sourceId}>{dataset.sourceId}</SelectItem>)}
        </SelectContent>
      </Select>
    </div>
  );
}

function SymbolDetailEditor({
  draft,
  role,
  value,
  constraint,
  priorDistribution,
  priorArgs,
  onValueChange,
  onConstraintChange,
  onPriorDistributionChange,
  onPriorArgsChange,
}: {
  draft: BayesModelDraftDTO;
  role: BayesSymbolRoleDTO;
  value: string;
  constraint: ParameterConstraintDTO;
  priorDistribution: PriorSpecDTO['distribution'];
  priorArgs: string[];
  onValueChange: (value: string) => void;
  onConstraintChange: (constraint: ParameterConstraintDTO) => void;
  onPriorDistributionChange: (distribution: PriorSpecDTO['distribution']) => void;
  onPriorArgsChange: (args: string[]) => void;
}) {
  if (role === 'parameter') {
    return (
      <div className="grid gap-2 md:grid-cols-[120px_160px_minmax(160px,1fr)]">
        <Select value={constraint.type} onValueChange={(value) => onConstraintChange({ type: value as ParameterConstraintDTO['type'] })}>
          <SelectTrigger size="sm"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="real">real</SelectItem>
            <SelectItem value="positive">positive</SelectItem>
            <SelectItem value="unit">unit</SelectItem>
          </SelectContent>
        </Select>
        <Select value={priorDistribution} onValueChange={(value) => onPriorDistributionChange(value as PriorSpecDTO['distribution'])}>
          <SelectTrigger size="sm"><SelectValue placeholder="选择分布" /></SelectTrigger>
          <SelectContent>
            {PRIOR_DISTRIBUTIONS.map(distribution => (
              <SelectItem key={distribution} value={distribution}>{priorDistributionLabel(distribution)}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="flex gap-1">
          {priorArgLabels(priorDistribution).map((label, index) => (
            <Input
              key={label}
              aria-label={label}
              value={priorArgs[index] ?? ''}
              className="h-7 min-w-0 font-mono"
              placeholder={label}
              onChange={(event) => onPriorArgsChange(replaceAt(priorArgs, index, event.target.value))}
            />
          ))}
        </div>
      </div>
    );
  }

  return (
    <Select value={value} onValueChange={onValueChange}>
      <SelectTrigger size="sm" className="w-48"><SelectValue placeholder="选择数据列" /></SelectTrigger>
      <SelectContent>
        {(draft.dataset?.columns ?? []).map(column => (
          <SelectItem key={column.name} value={column.name}>{column.name}</SelectItem>
        ))}
      </SelectContent>
    </Select>
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
    const parameter = draft.parameters.find(item => item.name === name);
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





function ResultSummaryContent({ result }: { result: InferenceResultDTO | null }) {
  return !result ? <p className="text-sm text-muted-foreground">运行完成后显示参数摘要。</p> : (
    <div className="rounded-md border border-border">
      <Table>
        <TableHeader>
          <TableRow><TableHead>parameter</TableHead><TableHead>mean</TableHead><TableHead>sd</TableHead><TableHead>2.5%</TableHead><TableHead>97.5%</TableHead><TableHead>rhat</TableHead><TableHead>ess</TableHead></TableRow>
        </TableHeader>
        <TableBody>
          {result.summaries.map(row => (
            <TableRow key={row.parameter}>
              <TableCell className="font-mono">{row.parameter}</TableCell>
              <TableCell>{formatNumber(row.mean)}</TableCell>
              <TableCell>{formatNumber(row.sd)}</TableCell>
              <TableCell>{formatNumber(row.q025)}</TableCell>
              <TableCell>{formatNumber(row.q975)}</TableCell>
              <TableCell>{row.rhat?.toFixed(3) ?? '—'}</TableCell>
              <TableCell>{Math.round(row.essBulk ?? 0)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

export function ResultOverview({ result }: { result: InferenceResultDTO | null }) {
  return (
    <section className="space-y-4">
      <Card>
        <CardHeader><PanelTitle title="Result Summary" description="标准化 InferenceResultDTO 展示" /></CardHeader>
        <CardContent>
          <ResultSummaryContent result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title="Diagnostics" description="不能只显示采样成功，必须展示诊断" /></CardHeader>
        <CardContent>
          <DiagnosticsContent result={result} />
        </CardContent>
      </Card>
    </section>
  );
}

function DiagnosticsContent({ result }: { result: InferenceResultDTO | null }) {
  return (
    <div className="space-y-2 text-sm">
      {!result ? <p className="text-muted-foreground">暂无诊断。</p> : (
        <>
          <IssueLine prefix="✓" issue={`Chains: ${result.diagnostics.chains}, draws per chain: ${result.diagnostics.drawsPerChain}`} />
          <IssueLine prefix="✓" issue={`Divergences: ${result.diagnostics.divergences ?? 0}`} />
          <IssueLine prefix="✓" issue={`Max treedepth hits: ${result.diagnostics.maxTreedepthHits ?? 0}`} />
        </>
      )}
    </div>
  );
}


function PanelTitle({ title, description }: { title: string; description: string }) {
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

function IssueLine({ prefix, issue }: { prefix: string; issue: string }) {
  return <p><span className="mr-2">{prefix}</span>{issue}</p>;
}

function LatexFormulaPreview({ formulaText }: { formulaText: string }) {
  const html = useMemo(() => renderFormulaLatex(formulaText), [formulaText]);
  return (
    <div
      className="rounded-md border border-border bg-muted/30 px-3 py-3 text-sm overflow-x-auto [&_.katex]:text-foreground [&_.katex-display]:my-0"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function renderFormulaLatex(formulaText: string): string {
  const latex = formulaText.trim() || '\\cdots';
  try {
    return katex.renderToString(latex, {
      displayMode: true,
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

function formatNumber(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : '—';
}

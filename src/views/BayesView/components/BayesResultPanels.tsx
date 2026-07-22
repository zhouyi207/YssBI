import type { ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { VscCloudDownload, VscFolderOpened } from 'react-icons/vsc';
import type { InferenceResultDTO, PosteriorPredictiveRowDTO, TraceSeriesDTO } from '@/shared/types/bayes';
import { KDEChart, MultiLineChart, PredictiveIntervalChart } from '@/shared/charts';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { diagnosticSeverityClass, evaluateInferenceDiagnostics, parameterDiagnosticLabel, parameterDiagnosticStatus } from '@/features/domain/bayes';
import { uiStore } from '@/features/core/ui/UIStore';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { exportBayesArtifactCsv, readBayesAutocorrelationData, readBayesDensityPlotData, readBayesPosteriorPredictive, readBayesTracePlotData, revealBayesResultFolder } from '@/services/bayes/bayesInferenceService';
import { LatexInline, PanelTitle, formatNumber, latexSymbol } from './BayesPanels';
import { useBayesPlotData } from './useBayesPlotData';

const RHAT_RATINGS = [
  ['1.000', '极好'],
  ['>1.000 且 <1.01', '推荐标准'],
  ['1.01–1.05', '有点问题'],
  ['>1.05–1.10', '不建议相信'],
  ['>1.10', '基本没收敛'],
] as const;

const ESS_RATINGS = [
  ['>2000', '非常好'],
  ['1001–2000', '很好'],
  ['400–1000', '可接受'],
  ['100–399', '偏低'],
  ['<100', '不可靠'],
] as const;

type DiagnosticWarningDescription = ReturnType<typeof evaluateInferenceDiagnostics>['warnings'][number];

export function rhatRating(value?: number): { label: string; className: string } {
  if (value == null) return { label: '不可用', className: 'text-muted-foreground' };
  if (value > 1.1) return { label: '基本没收敛', className: 'text-destructive' };
  if (value > 1.05) return { label: '不建议相信', className: 'text-destructive' };
  if (value >= 1.01) return { label: '有点问题', className: 'text-amber-500' };
  if (value > 1) return { label: '推荐标准', className: 'text-emerald-500' };
  return { label: '极好', className: 'text-emerald-500' };
}

export function essRating(value?: number): { label: string; className: string } {
  if (value == null) return { label: '不可用', className: 'text-muted-foreground' };
  if (value < 100) return { label: '不可靠', className: 'text-destructive' };
  if (value < 400) return { label: '偏低', className: 'text-amber-500' };
  if (value <= 1_000) return { label: '可接受', className: 'text-emerald-500' };
  if (value <= 2_000) return { label: '很好', className: 'text-emerald-500' };
  return { label: '非常好', className: 'text-emerald-500' };
}

function uniqueParameterWarnings(warnings: readonly DiagnosticWarningDescription[], parameter: string, code: string) {
  return warnings.filter(warning => warning.parameter === parameter && warning.code === code).slice(0, 1);
}

function DiagnosticMetricHeader({
  label,
  ratings,
  description,
}: {
  label: string;
  ratings: readonly (readonly [string, string])[];
  description?: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" className="cursor-help border-b border-dashed border-muted-foreground/60 uppercase">
          {label}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="w-72 flex-col items-stretch gap-2 p-3">
        {description ? <p className="border-b border-background/20 pb-2 leading-relaxed">{description}</p> : null}
        <div className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1 text-xs">
          {ratings.map(([range, meaning]) => (
            <div key={range} className="contents">
              <span className="font-mono">{range}</span>
              <span>{meaning}</span>
            </div>
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

function ParameterMetricValue({
  value,
  rating,
  details,
  warnings,
}: {
  value: string;
  rating: { label: string; className: string };
  details?: readonly string[];
  warnings: readonly DiagnosticWarningDescription[];
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" className={`cursor-help border-b border-dashed border-current font-mono ${rating.className}`}>
          {value}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-sm p-3">
        <div className="space-y-2">
          <p className="font-medium">评级：{rating.label}</p>
          {details?.map(detail => <p key={detail}>{detail}</p>)}
          {warnings.map(warning => (
            <div key={`${warning.parameter}-${warning.code}`} className="space-y-1 border-t border-background/20 pt-2">
              <p className="font-medium">{warning.title}</p>
              <p>{warning.explanation}</p>
              <p>建议：{warning.suggestion}</p>
            </div>
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

function ResultSummaryContent({
  result,
  assessment,
}: {
  result: InferenceResultDTO | null;
  assessment: ReturnType<typeof evaluateInferenceDiagnostics>;
}) {
  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示参数摘要。</p>;
  const diagnostics = result.diagnostics;
  const globalWarnings = assessment.warnings.filter(warning => !warning.parameter);

  return (
    <div className="space-y-4">
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-5">
        <SamplingMetric label="Chains" value={diagnostics.chains} />
        <SamplingMetric label="Draws / chain" value={diagnostics.drawsPerChain} />
        <SamplingMetric label="Warmup / chain" value={diagnostics.warmup} />
        <SamplingMetric
          label="Divergences"
          value={diagnostics.divergences ?? 'unavailable'}
          severity={(diagnostics.divergences ?? 0) > 0 ? 'bad' : diagnostics.divergences == null ? 'unknown' : 'good'}
        />
        <SamplingMetric
          label="Tree-depth hits"
          value={diagnostics.maxTreedepthHits ?? 'unavailable'}
          severity={(diagnostics.maxTreedepthHits ?? 0) > 0 ? 'warning' : diagnostics.maxTreedepthHits == null ? 'unknown' : 'good'}
        />
      </div>

      <div className="rounded-md border border-border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>status</TableHead><TableHead>parameter</TableHead><TableHead>mean</TableHead><TableHead>sd</TableHead><TableHead>2.5%</TableHead><TableHead>97.5%</TableHead>
              <TableHead>
                              <DiagnosticMetricHeader
                                label="R-hat"
                                ratings={RHAT_RATINGS}
                                description="衡量多条 MCMC chain 是否收敛到同一后验分布；数值越接近 1，链间混合通常越好。"
                              />
                            </TableHead>
              <TableHead>
                <DiagnosticMetricHeader
                  label="ESS bulk / tail"
                  ratings={ESS_RATINGS}
                  description="左侧为 Bulk ESS，右侧为 Tail ESS。单元格颜色与综合评级采用两者中较低的一项。"
                />
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {result.summaries.map(row => {
              const status = parameterDiagnosticStatus(row);
              const rhat = rhatRating(row.rhat);
              const essValues = [row.essBulk, row.essTail].filter((value): value is number => value != null);
              const ess = essRating(essValues.length > 0 ? Math.min(...essValues) : undefined);
              const bulkEss = row.essBulk == null ? '—' : String(Math.round(row.essBulk));
              const tailEss = row.essTail == null ? '—' : String(Math.round(row.essTail));
              return (
                <TableRow key={row.parameter}>
                  <TableCell className={parameterStatusClass(status)}>{parameterDiagnosticLabel(status)}</TableCell>
                  <TableCell><LatexInline formulaText={latexSymbol(row.parameter)} /></TableCell>
                  <TableCell>{formatNumber(row.mean)}</TableCell>
                  <TableCell>{formatNumber(row.sd)}</TableCell>
                  <TableCell>{formatNumber(row.q025)}</TableCell>
                  <TableCell>{formatNumber(row.q975)}</TableCell>
                  <TableCell>
                    <ParameterMetricValue
                      value={row.rhat?.toFixed(3) ?? '—'}
                      rating={rhat}
                      warnings={uniqueParameterWarnings(assessment.warnings, row.parameter, 'RHAT_TOO_HIGH')}
                    />
                  </TableCell>
                  <TableCell>
                    <ParameterMetricValue
                      value={`${bulkEss} / ${tailEss}`}
                      rating={ess}
                      details={[
                        `Bulk ESS：${bulkEss}（${essRating(row.essBulk).label}）`,
                        `Tail ESS：${tailEss}（${essRating(row.essTail).label}）`,
                      ]}
                      warnings={uniqueParameterWarnings(assessment.warnings, row.parameter, 'ESS_TOO_LOW')}
                    />
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </div>

      {globalWarnings.length > 0 ? <DiagnosticWarningList warnings={globalWarnings} /> : null}
      {assessment.severity !== 'good' && assessment.suggestions.length > 0 ? (
        <div className="space-y-1 rounded-md border border-border bg-muted/10 p-3">
          <p className="text-xs font-medium text-muted-foreground">Suggested next steps</p>
          <ul className="list-disc space-y-1 pl-5 text-xs text-muted-foreground">
            {assessment.suggestions.map(suggestion => <li key={suggestion}>{suggestion}</li>)}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

function SummaryDiagnosticBadge({ assessment }: { assessment: ReturnType<typeof evaluateInferenceDiagnostics> }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className={`rounded-sm border border-border px-2 py-1 text-xs font-medium uppercase ${diagnosticSeverityClass(assessment.severity)}`}
        >
          {assessment.severity}
        </button>
      </TooltipTrigger>
      <TooltipContent side="left" className="max-w-sm p-3">
        <div className="space-y-1">
          <p className="font-medium">{assessment.title}</p>
          <p>{assessment.summary}</p>
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

function SamplingMetric({ label, value, severity = 'good' }: { label: string; value: number | string; severity?: 'good' | 'warning' | 'bad' | 'unknown' }) {
  return (
    <div className="rounded-md border border-border bg-muted/10 p-3">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className={`mt-1 font-mono text-sm font-medium ${diagnosticSeverityClass(severity)}`}>{value}</p>
    </div>
  );
}

export function ResultOverview({ result }: { result: InferenceResultDTO | null }) {
  const artifactPath = result?.artifactManifest.artifacts[0]?.path;
  const assessment = evaluateInferenceDiagnostics(result);
  const [predictiveScale, setPredictiveScale] = useState<'original' | 'model'>('original');
  const [responseTransform, setResponseTransform] = useState<'identity' | 'ln'>('identity');

  useEffect(() => {
    setPredictiveScale('original');
    setResponseTransform('identity');
  }, [result?.artifactManifest.taskId]);
  const openResultFolder = () => {
    if (!artifactPath) return;
    void revealBayesResultFolder(artifactPath).catch(error => {
      uiStore.showToast(formatErrorMessage(error), 'error');
    });
  };

  return (
    <section className="space-y-4">
      <div className="flex justify-end">
        <Button size="sm" variant="outline" disabled={!artifactPath} onClick={openResultFolder}>
          <VscFolderOpened />
          打开结果文件夹
        </Button>
      </div>
      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <PanelTitle title="Result Summary" description="标准化 InferenceResultDTO 展示" />
          {result ? <SummaryDiagnosticBadge assessment={assessment} /> : null}
        </CardHeader>
        <CardContent>
          <ResultSummaryContent result={result} assessment={assessment} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <PanelTitle title="Posterior Trace" description="后端返回 trace 数据，前端负责渲染" />
          <BayesCsvExportButton
            result={result}
            kind="posterior_samples"
            fileName="posterior-samples.csv"
          />
        </CardHeader>
        <CardContent>
          <PosteriorTracePreview result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title="Posterior Density" description="后端返回 density 数据，前端负责渲染" /></CardHeader>
        <CardContent>
          <PosteriorDensityPreview result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title="Autocorrelation" description="后端返回 autocorrelation 数据，前端负责渲染" /></CardHeader>
        <CardContent>
          <AutocorrelationPreview result={result} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <div className="flex flex-wrap items-center gap-4">
            <PanelTitle title="Posterior Predictive" description="预测区间与后验预测数据" />
            {responseTransform !== 'identity' ? (
              <div className="flex items-center gap-2">
                <Label className="text-xs text-muted-foreground">Scale</Label>
                <Select value={predictiveScale} onValueChange={value => setPredictiveScale(value as 'original' | 'model')}>
                  <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="original">Original</SelectItem>
                    <SelectItem value="model">Model ({responseTransform})</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            ) : null}
          </div>
          <BayesCsvExportButton result={result} kind="posterior_predictive" fileName="posterior-predictive.csv" />
        </CardHeader>
        <CardContent>
          <PosteriorPredictivePreview
            result={result}
            scale={predictiveScale}
            onResponseTransform={setResponseTransform}
          />
        </CardContent>
      </Card>
    </section>
  );
}

function BayesCsvExportButton({
  result,
  kind,
  fileName,
  label = '导出 CSV',
}: {
  result: InferenceResultDTO | null;
  kind: 'posterior_samples' | 'posterior_predictive';
  fileName: string;
  label?: string;
}) {
  const available = Boolean(result && findArtifact(result, kind));
  const exportCsv = async () => {
    if (!result || !available) return;
    try {
      const destination = await save({
        title: '导出 CSV',
        defaultPath: fileName,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!destination) return;
      await exportBayesArtifactCsv(result.artifactManifest.taskId, kind, destination);
      uiStore.showToast('CSV 导出成功', 'success');
    } catch (error) {
      uiStore.showToast(formatErrorMessage(error), 'error');
    }
  };

  return (
    <Button size="sm" variant="outline" disabled={!available} onClick={() => void exportCsv()}>
      <VscCloudDownload />
      {label}
    </Button>
  );
}

function findArtifact(result: InferenceResultDTO, kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind']) {
  return result.artifactManifest.artifacts.find(artifact => artifact.kind === kind);
}

function artifactTaskId(result: InferenceResultDTO | null, kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind']): string | undefined {
  return result && findArtifact(result, kind) ? result.artifactManifest.taskId : undefined;
}



const loadTracePlot = (taskId: string, parameter: string) => readBayesTracePlotData(taskId, parameter, 500);
const loadDensityPlot = (taskId: string, parameter: string) => readBayesDensityPlotData(taskId, parameter, 256);
const loadAutocorrelationPlot = (taskId: string, parameter: string) => readBayesAutocorrelationData(taskId, parameter, 50);

export function traceChains(series: readonly TraceSeriesDTO[]): number[] {
  return Array.from(new Set(series.map(item => item.chain))).sort((left, right) => left - right);
}

export function filterTraceSeries(series: readonly TraceSeriesDTO[], selectedChain: string): TraceSeriesDTO[] {
  if (selectedChain === '__all__') return [...series];
  const chain = Number(selectedChain);
  return series.filter(item => item.chain === chain);
}

function PosteriorTracePreview({ result }: { result: InferenceResultDTO | null }) {
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadTracePlot);
  const [selectedChain, setSelectedChain] = useState('__all__');
  const chains = useMemo(() => traceChains(data?.series ?? []), [data]);
  const visibleSeries = filterTraceSeries(data?.series ?? [], selectedChain);
  const handleParameterChange = (nextParameter: string) => {
    setSelectedChain('__all__');
    setSelectedParameter(nextParameter);
  };

  useEffect(() => {
    setSelectedChain('__all__');
  }, [result?.artifactManifest.taskId]);

  useEffect(() => {
    if (selectedChain !== '__all__' && !chains.includes(Number(selectedChain))) {
      setSelectedChain('__all__');
    }
  }, [chains, selectedChain]);

  return (
    <PosteriorPlotFrame
      result={result}
      parameters={parameters}
      selectedParameter={parameter}
      loading={loading}
      error={error}
      onParameterChange={handleParameterChange}
      secondaryControl={(
        <div className="flex items-center gap-2">
          <Label className="text-xs text-muted-foreground">Chain</Label>
          <Select value={selectedChain} onValueChange={setSelectedChain}>
            <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all__">All chains ({chains.length})</SelectItem>
              {chains.map(chain => <SelectItem key={chain} value={String(chain)}>chain {chain}</SelectItem>)}
            </SelectContent>
          </Select>
        </div>
      )}
    >
      {data && (
        visibleSeries.some(item => item.points.length > 0) ? (
          <MultiLineChart
            series={visibleSeries.map(item => ({
              id: `${item.parameter}-${item.chain}`,
              label: `chain ${item.chain}`,
              points: item.points.map(point => ({ x: point.draw, y: point.value })),
            }))}
            xLabel={`draw, stride ${data.stride}`}
            yLabel="value"
          />
        ) : <p className="text-sm text-muted-foreground">没有 trace 数据。</p>
      )}
    </PosteriorPlotFrame>
  );
}

function PosteriorDensityPreview({ result }: { result: InferenceResultDTO | null }) {
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadDensityPlot);

  return (
    <PosteriorPlotFrame
      result={result}
      parameters={parameters}
      selectedParameter={parameter}
      loading={loading}
      error={error}
      onParameterChange={setSelectedParameter}
    >
      {data && (
        data.series.some(item => item.points.length > 0) ? (
          <KDEChart
            data={data.series.flatMap(item => item.points.map(point => ({ x: point.x, y: point.density })))}
            xLabel={parameter ?? 'value'}
            yLabel="Density"
            height={224}
            className="rounded-md border border-border bg-muted/10"
          />
        ) : <p className="text-sm text-muted-foreground">没有 density 数据。</p>
      )}
    </PosteriorPlotFrame>
  );
}

function AutocorrelationPreview({ result }: { result: InferenceResultDTO | null }) {
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadAutocorrelationPlot);

  return (
    <PosteriorPlotFrame
      result={result}
      parameters={parameters}
      selectedParameter={parameter}
      loading={loading}
      error={error}
      onParameterChange={setSelectedParameter}
    >
      {data && (
        data.series.some(item => item.points.length > 0) ? (
          <MultiLineChart
            series={data.series.map(item => ({
              id: `${item.parameter}-${item.chain}`,
              label: `chain ${item.chain}`,
              points: item.points.map(point => ({ x: point.lag, y: point.autocorrelation })),
            }))}
            xLabel={`lag, max ${data.maxLag}`}
            yLabel="autocorrelation"
            yDomain={[-1, 1]}
          />
        ) : <p className="text-sm text-muted-foreground">没有 autocorrelation 数据。</p>
      )}
    </PosteriorPlotFrame>
  );
}

function PosteriorPlotFrame({
  result,
  parameters,
  selectedParameter,
  loading,
  error,
  onParameterChange,
  secondaryControl,
  children,
}: {
  result: InferenceResultDTO | null;
  parameters: string[];
  selectedParameter?: string;
  loading: boolean;
  error: string | null;
  onParameterChange: (parameter: string) => void;
  secondaryControl?: ReactNode;
  children: ReactNode;
}) {
  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示图表数据。</p>;
  if (!findArtifact(result, 'posterior_samples')) return <p className="text-sm text-muted-foreground">当前结果没有保存 posterior samples，因此无法生成 trace / density / autocorrelation。请在 sampler 中启用 saveSamples 后重新运行。</p>;
  if (parameters.length === 0) return <p className="text-sm text-muted-foreground">没有可绘制的参数。</p>;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2">
          <Label className="text-xs text-muted-foreground">Parameter</Label>
          <Select value={selectedParameter} onValueChange={onParameterChange}>
            <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
            <SelectContent>
              {parameters.map(parameter => (
                              <SelectItem key={parameter} value={parameter} textValue={parameter}>
                                <LatexInline formulaText={latexSymbol(parameter)} />
                              </SelectItem>
                            ))}
            </SelectContent>
          </Select>
        </div>
        {secondaryControl}
      </div>
      {loading ? <p className="text-sm text-muted-foreground">正在读取 plot 数据...</p> : null}
      {error ? <p className="text-sm text-destructive">Plot 数据读取失败：{error}</p> : null}
      {!loading && !error ? children : null}
    </div>
  );
}





export function posteriorPredictiveChartData(
  rows: readonly PosteriorPredictiveRowDTO[],
  scale: 'original' | 'model',
) {
  return rows.map(row => {
    const summary = row[scale];
    return {
      observation: row.observation,
      observed: summary.observed,
      mean: summary.mean,
      lower: summary.q025,
      upper: summary.q975,
    };
  });
}

function PosteriorPredictivePreview({
  result,
  scale,
  onResponseTransform,
}: {
  result: InferenceResultDTO | null;
  scale: 'original' | 'model';
  onResponseTransform: (transform: 'identity' | 'ln') => void;
}) {
  const [plotRows, setPlotRows] = useState<PosteriorPredictiveRowDTO[]>([]);
  const [plotError, setPlotError] = useState<string | null>(null);
  const taskId = artifactTaskId(result, 'posterior_predictive');
  const artifactRows = result ? findArtifact(result, 'posterior_predictive')?.rows : undefined;

  useEffect(() => {
    setPlotRows([]);
    setPlotError(null);
    if (!taskId) return;

    let cancelled = false;
    readBayesPosteriorPredictive(taskId, 0, Math.max(artifactRows ?? 10_000, 1))
      .then(data => {
        if (cancelled) return;
        setPlotRows(data.rows);
        onResponseTransform(data.responseTransform);
      })
      .catch((caught: unknown) => {
        if (!cancelled) setPlotError(caught instanceof Error ? caught.message : String(caught));
      });
    return () => { cancelled = true; };
  }, [artifactRows, onResponseTransform, taskId]);

  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示 posterior predictive。</p>;
  if (!findArtifact(result, 'posterior_predictive')) return <p className="text-sm text-muted-foreground">当前结果没有保存 posterior predictive 数据。</p>;

  return (
    <div className="space-y-4">
      {plotRows.length > 0 ? (
        <PredictiveIntervalChart
          data={posteriorPredictiveChartData(plotRows, scale)}
          xLabel="observation"
          yLabel={scale === 'original' ? 'response' : 'response (model scale)'}
                  />
      ) : null}
      {plotError ? <p className="text-sm text-destructive">预测区间图读取失败：{plotError}</p> : null}
    </div>
  );
}



function DiagnosticWarningList({
  warnings,
}: {
  warnings: ReturnType<typeof evaluateInferenceDiagnostics>['warnings'];
}) {
  return (
    <div className="space-y-2 rounded-md border border-border bg-muted/10 p-3">
      <p className="text-xs font-medium text-muted-foreground">Warnings explained</p>
      <div className="space-y-2">
        {warnings.map((warning, index) => (
          <div key={`${warning.code}-${warning.parameter ?? 'global'}-${index}`} className="space-y-1 border-l-2 border-amber-500 pl-3 text-xs">
            <p><span className="font-mono text-amber-500">[{warning.code}]</span> <span className="font-medium text-foreground">{warning.title}</span></p>
            <p className="text-muted-foreground">{warning.explanation}</p>
            <p className="text-muted-foreground">建议：{warning.suggestion}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function parameterStatusClass(status: ReturnType<typeof parameterDiagnosticStatus>): string {
  switch (status) {
    case 'ok':
      return 'text-emerald-500';
    case 'check_rhat':
    case 'low_ess':
      return 'text-amber-500';
    case 'unknown':
      return 'text-muted-foreground';
  }
}


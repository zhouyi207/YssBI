import type { ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import type { InferenceResultDTO, PosteriorPredictivePageDTO, PosteriorSamplePageDTO } from '@/shared/types/bayes';
import { KDEChart, MultiLineChart } from '@/shared/charts';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { diagnosticSeverityClass, evaluateInferenceDiagnostics, parameterDiagnosticLabel, parameterDiagnosticStatus } from '@/features/domain/bayes';
import { readBayesAutocorrelationData, readBayesDensityPlotData, readBayesPosteriorPredictive, readBayesPosteriorSamples, readBayesTracePlotData } from '@/services/bayes/bayesInferenceService';
import { PanelTitle, formatNumber } from './BayesPanels';
import { useBayesPlotData } from './useBayesPlotData';

function ResultSummaryContent({ result }: { result: InferenceResultDTO | null }) {
  return !result ? <p className="text-sm text-muted-foreground">运行完成后显示参数摘要。</p> : (
    <div className="rounded-md border border-border">
      <Table>
        <TableHeader>
          <TableRow><TableHead>status</TableHead><TableHead>parameter</TableHead><TableHead>mean</TableHead><TableHead>sd</TableHead><TableHead>2.5%</TableHead><TableHead>97.5%</TableHead><TableHead>rhat</TableHead><TableHead>ess</TableHead></TableRow>
        </TableHeader>
        <TableBody>
          {result.summaries.map(row => {
            const status = parameterDiagnosticStatus(row);
            return (
              <TableRow key={row.parameter}>
                <TableCell className={parameterStatusClass(status)}>{parameterDiagnosticLabel(status)}</TableCell>
                <TableCell className="font-mono">{row.parameter}</TableCell>
                <TableCell>{formatNumber(row.mean)}</TableCell>
                <TableCell>{formatNumber(row.sd)}</TableCell>
                <TableCell>{formatNumber(row.q025)}</TableCell>
                <TableCell>{formatNumber(row.q975)}</TableCell>
                <TableCell>{row.rhat?.toFixed(3) ?? '—'}</TableCell>
                <TableCell>{Math.round(row.essBulk ?? 0)}</TableCell>
              </TableRow>
            );
          })}
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
      <Card>
        <CardHeader><PanelTitle title="Artifacts" description="后端产物可用性，用于后续图表和分页读取" /></CardHeader>
        <CardContent>
          <ArtifactAvailability result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title="Posterior Trace" description="后端返回 trace 数据，前端负责渲染" /></CardHeader>
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
        <CardHeader><PanelTitle title="Posterior Samples" description="通过后端分页读取 samples Arrow" /></CardHeader>
        <CardContent>
          <PosteriorSamplesPreview result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title="Posterior Predictive" description="通过后端分页读取 posterior predictive 数据" /></CardHeader>
        <CardContent>
          <PosteriorPredictivePreview result={result} />
        </CardContent>
      </Card>
    </section>
  );
}

function findArtifact(result: InferenceResultDTO, kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind']) {
  return result.artifactManifest.artifacts.find(artifact => artifact.kind === kind);
}

function artifactTaskId(result: InferenceResultDTO | null, kind: InferenceResultDTO['artifactManifest']['artifacts'][number]['kind']): string | undefined {
  return result && findArtifact(result, kind) ? result.artifactManifest.taskId : undefined;
}

function ArtifactAvailability({ result }: { result: InferenceResultDTO | null }) {
  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示后端产物状态。</p>;
  const manifest = result.artifactManifest;

  return (
    <div className="space-y-3">
      <div className="grid gap-2 text-sm md:grid-cols-4">
        <ArtifactBadge label="Summary" artifact={findArtifact(result, 'summary')} />
        <ArtifactBadge label="Posterior samples" artifact={findArtifact(result, 'posterior_samples')} />
        <ArtifactBadge label="Posterior predictive" artifact={findArtifact(result, 'posterior_predictive')} />
        <ArtifactBadge label="Log" artifact={findArtifact(result, 'log')} />
      </div>
      {manifest.artifacts.length ? (
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>kind</TableHead><TableHead>format</TableHead><TableHead>rows</TableHead><TableHead>path</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {manifest.artifacts.map(artifact => (
                <TableRow key={`${artifact.kind}-${artifact.path}`}>
                  <TableCell className="font-mono">{artifact.kind}</TableCell>
                  <TableCell className="font-mono">{artifact.format}</TableCell>
                  <TableCell>{artifact.rows ?? '—'}</TableCell>
                  <TableCell className="max-w-[360px] truncate font-mono text-xs" title={artifact.path}>{artifact.path}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      ) : null}
    </div>
  );
}

function ArtifactBadge({ label, artifact }: { label: string; artifact?: InferenceResultDTO['artifactManifest']['artifacts'][number] }) {
  const available = Boolean(artifact);
  const detail = artifact?.path;
  return (
    <div className="rounded-md border border-border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-foreground">{label}</span>
        <span className={available ? 'text-emerald-500' : 'text-muted-foreground'}>{available ? 'available' : 'unavailable'}</span>
      </div>
      {detail ? <p className="mt-1 truncate text-xs text-muted-foreground" title={detail}>{detail}</p> : null}
    </div>
  );
}

const loadTracePlot = (taskId: string, parameter: string) => readBayesTracePlotData(taskId, parameter, 500);
const loadDensityPlot = (taskId: string, parameter: string) => readBayesDensityPlotData(taskId, parameter, 256);
const loadAutocorrelationPlot = (taskId: string, parameter: string) => readBayesAutocorrelationData(taskId, parameter, 50);

function PosteriorTracePreview({ result }: { result: InferenceResultDTO | null }) {
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadTracePlot);

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
  children,
}: {
  result: InferenceResultDTO | null;
  parameters: string[];
  selectedParameter?: string;
  loading: boolean;
  error: string | null;
  onParameterChange: (parameter: string) => void;
  children: ReactNode;
}) {
  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示图表数据。</p>;
  if (!findArtifact(result, 'posterior_samples')) return <p className="text-sm text-muted-foreground">当前结果没有保存 posterior samples，因此无法生成 trace / density / autocorrelation。请在 sampler 中启用 saveSamples 后重新运行。</p>;
  if (parameters.length === 0) return <p className="text-sm text-muted-foreground">没有可绘制的参数。</p>;

  return (
    <div className="space-y-3">
      <div className="flex max-w-xs items-center gap-2">
        <Label className="text-xs text-muted-foreground">Parameter</Label>
        <Select value={selectedParameter} onValueChange={onParameterChange}>
          <SelectTrigger size="sm"><SelectValue /></SelectTrigger>
          <SelectContent>
            {parameters.map(parameter => <SelectItem key={parameter} value={parameter}>{parameter}</SelectItem>)}
          </SelectContent>
        </Select>
      </div>
      {loading ? <p className="text-sm text-muted-foreground">正在读取 plot 数据...</p> : null}
      {error ? <p className="text-sm text-destructive">Plot 数据读取失败：{error}</p> : null}
      {!loading && !error ? children : null}
    </div>
  );
}



function PosteriorSamplesPreview({ result }: { result: InferenceResultDTO | null }) {
  const pageSize = 20;
  const [page, setPage] = useState<PosteriorSamplePageDTO | null>(null);
  const [offset, setOffset] = useState(0);
  const [selectedParameter, setSelectedParameter] = useState<string>('__all__');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const taskId = artifactTaskId(result, 'posterior_samples');
  const parameters = useMemo(() => result?.summaries.map(summary => summary.parameter) ?? [], [result]);
  const parameterFilter = selectedParameter === '__all__' ? undefined : selectedParameter;

  useEffect(() => {
    setOffset(0);
  }, [taskId, selectedParameter]);

  useEffect(() => {
    let cancelled = false;
    setPage(null);
    setError(null);
    if (!taskId) return;

    setLoading(true);
    readBayesPosteriorSamples(taskId, offset, pageSize, parameterFilter)
      .then(nextPage => {
        if (!cancelled) setPage(nextPage);
      })
      .catch((caught: unknown) => {
        if (!cancelled) setError(caught instanceof Error ? caught.message : String(caught));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [taskId, offset, parameterFilter]);

  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示后验样本。</p>;
  if (!findArtifact(result, 'posterior_samples')) return <p className="text-sm text-muted-foreground">当前结果没有保存 posterior samples。请在 sampler 中启用 saveSamples 后重新运行。</p>;

  const total = page?.total ?? 0;
  const pageStart = total === 0 ? 0 : offset + 1;
  const pageEnd = Math.min(offset + pageSize, total);
  const canGoPrevious = offset > 0 && !loading;
  const canGoNext = pageEnd < total && !loading;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-48 items-center gap-2">
          <Label className="text-xs text-muted-foreground">Parameter</Label>
          <Select value={selectedParameter} onValueChange={setSelectedParameter}>
            <SelectTrigger size="sm"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all__">All parameters</SelectItem>
              {parameters.map(parameter => <SelectItem key={parameter} value={parameter}>{parameter}</SelectItem>)}
            </SelectContent>
          </Select>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={!canGoPrevious} onClick={() => setOffset(current => Math.max(0, current - pageSize))}>上一页</Button>
          <Button size="sm" variant="outline" disabled={!canGoNext} onClick={() => setOffset(current => current + pageSize)}>下一页</Button>
        </div>
      </div>
      {loading ? <p className="text-sm text-muted-foreground">正在读取 posterior samples...</p> : null}
      {error ? <p className="text-sm text-destructive">Samples 读取失败：{error}</p> : null}
      {!loading && !error && (!page || page.rows.length === 0) ? <p className="text-sm text-muted-foreground">没有可显示的 posterior samples。</p> : null}
      {page && page.rows.length > 0 ? (
        <>
          <p className="text-xs text-muted-foreground">显示 {pageStart}-{pageEnd} / {page.total} 条 draws。</p>
          <div className="rounded-md border border-border">
            <Table>
              <TableHeader>
                <TableRow><TableHead>parameter</TableHead><TableHead>chain</TableHead><TableHead>draw</TableHead><TableHead>value</TableHead></TableRow>
              </TableHeader>
              <TableBody>
                {page.rows.map((row, index) => (
                  <TableRow key={`${row.parameter}-${row.chain}-${row.draw}-${index}`}>
                    <TableCell className="font-mono">{row.parameter}</TableCell>
                    <TableCell>{row.chain}</TableCell>
                    <TableCell>{row.draw}</TableCell>
                    <TableCell>{formatNumber(row.value)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </>
      ) : null}
    </div>
  );
}

function PosteriorPredictivePreview({ result }: { result: InferenceResultDTO | null }) {
  const pageSize = 20;
  const [page, setPage] = useState<PosteriorPredictivePageDTO | null>(null);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const taskId = artifactTaskId(result, 'posterior_predictive');

  useEffect(() => {
    setOffset(0);
  }, [taskId]);

  useEffect(() => {
    let cancelled = false;
    setPage(null);
    setError(null);
    if (!taskId) return;

    setLoading(true);
    readBayesPosteriorPredictive(taskId, offset, pageSize)
      .then(nextPage => {
        if (!cancelled) setPage(nextPage);
      })
      .catch((caught: unknown) => {
        if (!cancelled) setError(caught instanceof Error ? caught.message : String(caught));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [taskId, offset]);

  if (!result) return <p className="text-sm text-muted-foreground">运行完成后显示 posterior predictive。</p>;
  if (!findArtifact(result, 'posterior_predictive')) return <p className="text-sm text-muted-foreground">当前结果没有保存 posterior predictive 数据。</p>;

  const total = page?.total ?? 0;
  const pageStart = total === 0 ? 0 : offset + 1;
  const pageEnd = Math.min(offset + pageSize, total);
  const canGoPrevious = offset > 0 && !loading;
  const canGoNext = pageEnd < total && !loading;

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs text-muted-foreground">显示 {pageStart}-{pageEnd} / {total} 条 posterior predictive rows。</p>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={!canGoPrevious} onClick={() => setOffset(current => Math.max(0, current - pageSize))}>上一页</Button>
          <Button size="sm" variant="outline" disabled={!canGoNext} onClick={() => setOffset(current => current + pageSize)}>下一页</Button>
        </div>
      </div>
      {loading ? <p className="text-sm text-muted-foreground">正在读取 posterior predictive...</p> : null}
      {error ? <p className="text-sm text-destructive">Posterior predictive 读取失败：{error}</p> : null}
      {!loading && !error && (!page || page.rows.length === 0) ? <p className="text-sm text-muted-foreground">没有可显示的 posterior predictive 数据。</p> : null}
      {page && page.rows.length > 0 ? (
        <div className="rounded-md border border-border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>observation</TableHead><TableHead>observed</TableHead><TableHead>mean</TableHead><TableHead>2.5%</TableHead><TableHead>97.5%</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {page.rows.map(row => (
                <TableRow key={row.observation}>
                  <TableCell>{row.observation}</TableCell>
                  <TableCell>{formatNumber(row.observed)}</TableCell>
                  <TableCell>{formatNumber(row.mean)}</TableCell>
                  <TableCell>{formatNumber(row.q025)}</TableCell>
                  <TableCell>{formatNumber(row.q975)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      ) : null}
    </div>
  );
}

function DiagnosticsContent({ result }: { result: InferenceResultDTO | null }) {
  if (!result) return <p className="text-sm text-muted-foreground">暂无诊断。</p>;

  const assessment = evaluateInferenceDiagnostics(result);


  return (
    <div className="space-y-4 text-sm">
      <div className="rounded-md border border-border bg-muted/10 p-3">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className={`font-medium ${diagnosticSeverityClass(assessment.severity)}`}>{assessment.title}</p>
            <p className="mt-1 text-muted-foreground">{assessment.summary}</p>
          </div>
          <span className={`rounded-sm border border-border px-2 py-1 text-xs font-medium uppercase ${diagnosticSeverityClass(assessment.severity)}`}>
            {assessment.severity}
          </span>
        </div>
      </div>

      <div className="space-y-2">
        {assessment.metrics.map(metric => (
          <IssueLine key={metric.key} prefix={metric.severity === 'good' ? '✓' : '•'} issue={metric.label} />
        ))}
      </div>

      {assessment.warnings.length > 0 ? <DiagnosticWarningList warnings={assessment.warnings} /> : <IssueLine prefix="✓" issue="No diagnostic warnings reported." />}

      {assessment.suggestions.length > 0 ? (
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

function IssueLine({ prefix, issue }: { prefix: string; issue: string }) {
  return <p><span className="mr-2">{prefix}</span>{issue}</p>;
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


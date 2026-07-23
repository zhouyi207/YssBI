import type { ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { save } from '@tauri-apps/plugin-dialog';
import { VscCloudDownload, VscFolderOpened } from 'react-icons/vsc';
import type { AutocorrelationSeriesDTO, DensitySeriesDTO, InferenceResultDTO, PosteriorPredictiveRowDTO, TraceSeriesDTO } from '@/shared/types/bayes';
import { MultiLineChart, PredictiveIntervalChart } from '@/shared/charts';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { diagnosticSeverityClass, evaluateInferenceDiagnostics, parameterDiagnosticStatus } from '@/features/domain/bayes';
import { uiStore } from '@/features/core/ui/UIStore';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { exportBayesArtifactCsv, readBayesAutocorrelationData, readBayesDensityPlotData, readBayesPosteriorPredictive, readBayesTracePlotData, revealBayesResultFolder } from '@/services/bayes/bayesInferenceService';
import { LatexInline, PanelTitle, formatNumber, latexSymbol } from './BayesPanels';
import { useBayesPlotData } from './useBayesPlotData';

type RatingCode = 'unavailable' | 'excellent' | 'recommended' | 'concerning' | 'untrustworthy' | 'notConverged'
  | 'veryGood' | 'good' | 'acceptable' | 'low' | 'unreliable';

type DiagnosticWarningDescription = ReturnType<typeof evaluateInferenceDiagnostics>['warnings'][number];

export function rhatRating(value?: number): { code: RatingCode; className: string } {
  if (value == null) return { code: 'unavailable', className: 'text-muted-foreground' };
  if (value > 1.1) return { code: 'notConverged', className: 'text-destructive' };
  if (value > 1.05) return { code: 'untrustworthy', className: 'text-destructive' };
  if (value >= 1.01) return { code: 'concerning', className: 'text-amber-500' };
  if (value > 1) return { code: 'recommended', className: 'text-emerald-500' };
  return { code: 'excellent', className: 'text-emerald-500' };
}

export function essRating(value?: number): { code: RatingCode; className: string } {
  if (value == null) return { code: 'unavailable', className: 'text-muted-foreground' };
  if (value < 100) return { code: 'unreliable', className: 'text-destructive' };
  if (value < 400) return { code: 'low', className: 'text-amber-500' };
  if (value <= 1_000) return { code: 'acceptable', className: 'text-emerald-500' };
  if (value <= 2_000) return { code: 'good', className: 'text-emerald-500' };
  return { code: 'veryGood', className: 'text-emerald-500' };
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
  rating: { code: RatingCode; className: string };
  details?: readonly string[];
  warnings: readonly DiagnosticWarningDescription[];
}) {
  const { t } = useTranslation();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" className={`cursor-help border-b border-dashed border-current font-mono ${rating.className}`}>
          {value}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-sm p-3">
        <div className="space-y-2">
          <p className="font-medium">{t('bayes.results.diagnostics.ratingPrefix', { rating: t(`bayes.results.ratings.${rating.code}`) })}</p>
          {details?.map(detail => <p key={detail}>{detail}</p>)}
          {warnings.map(warning => (
            <div key={`${warning.parameter}-${warning.code}`} className="space-y-1 border-t border-background/20 pt-2">
              <p className="font-medium">{t(`bayes.results.diagnostics.warnings.${warning.code}.title`, { parameter: warning.parameter })}</p>
              <p>{t(`bayes.results.diagnostics.warnings.${warning.code}.explanation`)}</p>
              <p>{t('bayes.results.diagnostics.suggestionPrefix', { suggestion: t(`bayes.results.diagnostics.warnings.${warning.code}.suggestion`) })}</p>
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
  const { t } = useTranslation();
  if (!result) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.summary')}</p>;
  const diagnostics = result.diagnostics;
  const rhatRatings = [
    ['1.000', t('bayes.results.ratings.excellent')],
    ['>1.000 & <1.01', t('bayes.results.ratings.recommended')],
    ['1.01–1.05', t('bayes.results.ratings.concerning')],
    ['>1.05–1.10', t('bayes.results.ratings.untrustworthy')],
    ['>1.10', t('bayes.results.ratings.notConverged')],
  ] as const;
  const essRatings = [
    ['>2000', t('bayes.results.ratings.veryGood')],
    ['1001–2000', t('bayes.results.ratings.good')],
    ['400–1000', t('bayes.results.ratings.acceptable')],
    ['100–399', t('bayes.results.ratings.low')],
    ['<100', t('bayes.results.ratings.unreliable')],
  ] as const;
  const globalWarnings = assessment.warnings.filter(warning => !warning.parameter);

  return (
    <div className="space-y-4">
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-5">
        <SamplingMetric label={t('bayes.results.metrics.chains')} value={diagnostics.chains} />
                <SamplingMetric label={t('bayes.results.metrics.drawsPerChain')} value={diagnostics.drawsPerChain} />
                <SamplingMetric label={t('bayes.results.metrics.warmupPerChain')} value={diagnostics.warmup} />
        <SamplingMetric
          label={t('bayes.results.metrics.divergences')}
                    value={diagnostics.divergences ?? t('bayes.results.ratings.unavailable')}
          severity={(diagnostics.divergences ?? 0) > 0 ? 'bad' : diagnostics.divergences == null ? 'unknown' : 'good'}
        />
        <SamplingMetric
          label={t('bayes.results.metrics.treeDepthHits')}
                    value={diagnostics.maxTreedepthHits ?? t('bayes.results.ratings.unavailable')}
          severity={(diagnostics.maxTreedepthHits ?? 0) > 0 ? 'warning' : diagnostics.maxTreedepthHits == null ? 'unknown' : 'good'}
        />
      </div>

      <div className="rounded-md border border-border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t('bayes.results.table.status')}</TableHead><TableHead>{t('bayes.results.table.parameter')}</TableHead><TableHead>{t('bayes.results.table.mean')}</TableHead><TableHead>{t('bayes.results.table.sd')}</TableHead><TableHead>2.5%</TableHead><TableHead>97.5%</TableHead>
              <TableHead>
                              <DiagnosticMetricHeader
                                label="R-hat"
                                ratings={rhatRatings}
                                                                description={t('bayes.results.diagnostics.rhatDescription')}
                              />
                            </TableHead>
              <TableHead>
                <DiagnosticMetricHeader
                  label="ESS bulk / tail"
                  ratings={essRatings}
                                    description={t('bayes.results.diagnostics.essDescription')}
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
                  <TableCell className={parameterStatusClass(status)}>{t(`bayes.results.status.${status}`)}</TableCell>
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
                        t('bayes.results.diagnostics.essDetail', { metric: 'Bulk ESS', value: bulkEss, rating: t(`bayes.results.ratings.${essRating(row.essBulk).code}`) }),
                                                t('bayes.results.diagnostics.essDetail', { metric: 'Tail ESS', value: tailEss, rating: t(`bayes.results.ratings.${essRating(row.essTail).code}`) }),
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
          <p className="text-xs font-medium text-muted-foreground">{t('bayes.results.diagnostics.suggestedNextSteps')}</p>
          <ul className="list-disc space-y-1 pl-5 text-xs text-muted-foreground">
            {diagnosticSuggestionKeys(assessment.severity).map(key => <li key={key}>{t(key)}</li>)}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

function diagnosticSuggestionKeys(severity: ReturnType<typeof evaluateInferenceDiagnostics>['severity']): string[] {
  if (severity === 'unknown') return ['bayes.results.diagnostics.suggestions.checkMetrics', 'bayes.results.diagnostics.suggestions.saveSamples'];
  return ['bayes.results.diagnostics.suggestions.increaseSampling', 'bayes.results.diagnostics.suggestions.inspectPlots'];
}

function SummaryDiagnosticBadge({ assessment }: { assessment: ReturnType<typeof evaluateInferenceDiagnostics> }) {
  const { t } = useTranslation();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className={`rounded-sm border border-border px-2 py-1 text-xs font-medium uppercase ${diagnosticSeverityClass(assessment.severity)}`}
        >
          {t(`bayes.results.severity.${assessment.severity}`)}
        </button>
      </TooltipTrigger>
      <TooltipContent side="left" className="max-w-sm p-3">
        <div className="space-y-1">
          <p className="font-medium">{t(`bayes.results.diagnostics.assessment.${assessment.severity}.title`)}</p>
          <p>{t(`bayes.results.diagnostics.assessment.${assessment.severity}.summary`)}</p>
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
  const { t } = useTranslation();
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
          uiStore.showToast(t('bayes.results.errors.openFolder', { error: formatErrorMessage(error) }), 'error');
        });
  };

  return (
    <section className="space-y-4">
      <div className="flex justify-end">
        <Button size="sm" variant="outline" disabled={!artifactPath} onClick={openResultFolder}>
          <VscFolderOpened />
          {t('bayes.results.actions.openFolder')}
        </Button>
      </div>
      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <PanelTitle title={t('bayes.results.titles.summary')} />
          {result ? <SummaryDiagnosticBadge assessment={assessment} /> : null}
        </CardHeader>
        <CardContent>
          <ResultSummaryContent result={result} assessment={assessment} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <PanelTitle title={t('bayes.results.titles.posteriorTrace')} />
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
        <CardHeader><PanelTitle title={t('bayes.results.titles.posteriorDensity')} /></CardHeader>
        <CardContent>
          <PosteriorDensityPreview result={result} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader><PanelTitle title={t('bayes.results.titles.autocorrelation')} /></CardHeader>
        <CardContent>
          <AutocorrelationPreview result={result} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <div className="flex flex-wrap items-center gap-4">
            <PanelTitle title={t('bayes.results.titles.posteriorPredictive')} />
            {responseTransform !== 'identity' ? (
              <div className="flex items-center gap-2">
                <Label className="text-xs text-muted-foreground">{t('bayes.results.controls.scale')}</Label>
                <Select value={predictiveScale} onValueChange={value => setPredictiveScale(value as 'original' | 'model')}>
                  <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="original">{t('bayes.results.controls.originalScale')}</SelectItem>
                                        <SelectItem value="model">{t('bayes.results.controls.modelScale', { transform: responseTransform })}</SelectItem>
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
  label,
}: {
  result: InferenceResultDTO | null;
  kind: 'posterior_samples' | 'posterior_predictive';
  fileName: string;
  label?: string;
}) {
  const { t } = useTranslation();
  const available = Boolean(result && findArtifact(result, kind));
  const exportCsv = async () => {
    if (!result || !available) return;
    try {
      const destination = await save({
        title: t('bayes.results.actions.exportCsv'),
        defaultPath: fileName,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!destination) return;
      await exportBayesArtifactCsv(result.artifactManifest.taskId, kind, destination);
      uiStore.showToast(t('bayes.results.messages.exportSuccess'), 'success');
          } catch (error) {
            uiStore.showToast(t('bayes.results.errors.exportCsv', { error: formatErrorMessage(error) }), 'error');
    }
  };

  return (
    <Button size="sm" variant="outline" disabled={!available} onClick={() => void exportCsv()}>
      <VscCloudDownload />
      {label ?? t('bayes.results.actions.exportCsv')}
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
  const { t } = useTranslation();
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
          <Label className="text-xs text-muted-foreground">{t('bayes.results.controls.chain')}</Label>
          <Select value={selectedChain} onValueChange={setSelectedChain}>
            <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all__">{t('bayes.results.controls.allChains', { count: chains.length })}</SelectItem>
                            {chains.map(chain => <SelectItem key={chain} value={String(chain)}>{t('bayes.results.controls.chainNumber', { chain })}</SelectItem>)}
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
                            label: t('bayes.results.controls.chainNumber', { chain: item.chain }),
                            points: item.points.map(point => ({ x: point.draw, y: point.value })),
            }))}
            xLabel={t('bayes.results.chart.drawStride', { stride: data.stride })}
                        yLabel={t('bayes.results.chart.value')}
          />
        ) : <p className="text-sm text-muted-foreground">{t('bayes.results.empty.trace')}</p>
      )}
    </PosteriorPlotFrame>
  );
}

function PosteriorDensityPreview({ result }: { result: InferenceResultDTO | null }) {
  const { t } = useTranslation();
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadDensityPlot);
  const [selectedChain, setSelectedChain] = useState('__all__');
  const chains = useMemo(() => seriesChains(data?.series ?? []), [data]);
  const visibleSeries = filterDensitySeries(data?.series ?? [], selectedChain);
  const handleParameterChange = (nextParameter: string) => {
    setSelectedChain('__all__');
    setSelectedParameter(nextParameter);
  };
  useResetChainSelection(result, chains, selectedChain, setSelectedChain);

  return (
    <PosteriorPlotFrame
      result={result}
      parameters={parameters}
      selectedParameter={parameter}
      loading={loading}
      error={error}
      onParameterChange={handleParameterChange}
      secondaryControl={<ChainSelector value={selectedChain} chains={chains} includePooled onChange={setSelectedChain} />}
    >
      {data && (
        visibleSeries.some(item => item.points.length > 0) ? (
          <MultiLineChart
            series={visibleSeries.map(item => ({
              id: `${item.parameter}-${item.chain ?? 'pooled'}`,
              label: item.chain == null
                ? t('bayes.results.controls.pooled')
                : t('bayes.results.controls.chainNumber', { chain: item.chain }),
              points: item.points.map(point => ({ x: point.x, y: point.density })),
            }))}
            xLabel={parameter ?? t('bayes.results.chart.value')}
            yLabel={t('bayes.results.chart.density')}
          />
        ) : <p className="text-sm text-muted-foreground">{t('bayes.results.empty.density')}</p>
      )}
    </PosteriorPlotFrame>
  );
}

function AutocorrelationPreview({ result }: { result: InferenceResultDTO | null }) {
  const { t } = useTranslation();
  const { data, loading, error, parameters, parameter, setSelectedParameter } = useBayesPlotData(result, loadAutocorrelationPlot);
  const [selectedChain, setSelectedChain] = useState('__all__');
  const chains = useMemo(() => seriesChains(data?.series ?? []), [data]);
  const visibleSeries = filterChainSeries(data?.series ?? [], selectedChain);
  const handleParameterChange = (nextParameter: string) => {
    setSelectedChain('__all__');
    setSelectedParameter(nextParameter);
  };
  useResetChainSelection(result, chains, selectedChain, setSelectedChain);

  return (
    <PosteriorPlotFrame
      result={result}
      parameters={parameters}
      selectedParameter={parameter}
      loading={loading}
      error={error}
      onParameterChange={handleParameterChange}
      secondaryControl={<ChainSelector value={selectedChain} chains={chains} onChange={setSelectedChain} />}
    >
      {data && (
        visibleSeries.some(item => item.points.length > 0) ? (
          <MultiLineChart
            series={visibleSeries.map(item => ({
              id: `${item.parameter}-${item.chain}`,
              label: t('bayes.results.controls.chainNumber', { chain: item.chain }),
              points: item.points.map(point => ({ x: point.lag, y: point.autocorrelation })),
            }))}
            xLabel={t('bayes.results.chart.lagMax', { maxLag: data.maxLag })}
            yLabel={t('bayes.results.chart.autocorrelation')}
            yDomain={[-1, 1]}
          />
        ) : <p className="text-sm text-muted-foreground">{t('bayes.results.empty.autocorrelation')}</p>
      )}
    </PosteriorPlotFrame>
  );
}

function seriesChains(series: readonly { chain: number | null }[]): number[] {
  return Array.from(new Set(
    series.flatMap(item => item.chain == null ? [] : [item.chain]),
  )).sort((left, right) => left - right);
}

export function filterDensitySeries(
  series: readonly DensitySeriesDTO[],
  selectedChain: string,
): DensitySeriesDTO[] {
  if (selectedChain === '__pooled__') return series.filter(item => item.chain == null);
  if (selectedChain === '__all__') return series.filter(item => item.chain != null);
  const chain = Number(selectedChain);
  return series.filter(item => item.chain === chain);
}

export function filterAutocorrelationSeries(
  series: readonly AutocorrelationSeriesDTO[],
  selectedChain: string,
): AutocorrelationSeriesDTO[] {
  return filterChainSeries(series, selectedChain);
}

function filterChainSeries<T extends { chain: number }>(series: readonly T[], selectedChain: string): T[] {
  if (selectedChain === '__all__') return [...series];
  const chain = Number(selectedChain);
  return series.filter(item => item.chain === chain);
}

function useResetChainSelection(
  result: InferenceResultDTO | null,
  chains: readonly number[],
  selectedChain: string,
  setSelectedChain: (value: string) => void,
) {
  useEffect(() => {
    setSelectedChain('__all__');
  }, [result?.artifactManifest.taskId, setSelectedChain]);

  useEffect(() => {
    if (!selectedChain.startsWith('__') && !chains.includes(Number(selectedChain))) {
      setSelectedChain('__all__');
    }
  }, [chains, selectedChain, setSelectedChain]);
}

function ChainSelector({
  value,
  chains,
  includePooled = false,
  onChange,
}: {
  value: string;
  chains: readonly number[];
  includePooled?: boolean;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2">
      <Label className="text-xs text-muted-foreground">{t('bayes.results.controls.chain')}</Label>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger size="sm" className="w-36"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="__all__">{t('bayes.results.controls.allChains', { count: chains.length })}</SelectItem>
          {includePooled ? <SelectItem value="__pooled__">{t('bayes.results.controls.pooled')}</SelectItem> : null}
          {chains.map(chain => (
            <SelectItem key={chain} value={String(chain)}>
              {t('bayes.results.controls.chainNumber', { chain })}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
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
  const { t } = useTranslation();
  if (!result) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.plots')}</p>;
  if (!findArtifact(result, 'posterior_samples')) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.posteriorSamples')}</p>;
  if (parameters.length === 0) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.parameters')}</p>;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2">
          <Label className="text-xs text-muted-foreground">{t('bayes.results.controls.parameter')}</Label>
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
      {loading ? <p className="text-sm text-muted-foreground">{t('bayes.results.loading.plot')}</p> : null}
            {error ? <p className="text-sm text-destructive">{t('bayes.results.errors.plot', { error })}</p> : null}
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
  const { t } = useTranslation();
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

  if (!result) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.posteriorPredictive')}</p>;
    if (!findArtifact(result, 'posterior_predictive')) return <p className="text-sm text-muted-foreground">{t('bayes.results.empty.posteriorPredictiveData')}</p>;

  return (
    <div className="space-y-4">
      {plotRows.length > 0 ? (
        <PredictiveIntervalChart
          data={posteriorPredictiveChartData(plotRows, scale)}
          xLabel={t('bayes.results.chart.observation')}
                    yLabel={scale === 'original' ? t('bayes.results.chart.response') : t('bayes.results.chart.responseModelScale')}
                  />
      ) : null}
      {plotError ? <p className="text-sm text-destructive">{t('bayes.results.errors.predictivePlot', { error: plotError })}</p> : null}
    </div>
  );
}



function DiagnosticWarningList({
  warnings,
}: {
  warnings: ReturnType<typeof evaluateInferenceDiagnostics>['warnings'];
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-2 rounded-md border border-border bg-muted/10 p-3">
      <p className="text-xs font-medium text-muted-foreground">{t('bayes.results.diagnostics.warningsExplained')}</p>
      <div className="space-y-2">
        {warnings.map((warning, index) => (
          <div key={`${warning.code}-${warning.parameter ?? 'global'}-${index}`} className="space-y-1 border-l-2 border-amber-500 pl-3 text-xs">
            <p><span className="font-mono text-amber-500">[{warning.code}]</span> <span className="font-medium text-foreground">{t(`bayes.results.diagnostics.warnings.${warning.code}.title`, { parameter: warning.parameter })}</span></p>
                        <p className="text-muted-foreground">{t(`bayes.results.diagnostics.warnings.${warning.code}.explanation`)}</p>
                        <p className="text-muted-foreground">{t('bayes.results.diagnostics.suggestionPrefix', { suggestion: t(`bayes.results.diagnostics.warnings.${warning.code}.suggestion`) })}</p>
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


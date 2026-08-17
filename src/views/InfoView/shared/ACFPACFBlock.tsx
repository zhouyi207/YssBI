import React, { useState } from 'react';
import { Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { computeAcfPacf } from '@/features/application/stats/statsActions';
import { formatInlineUserError } from '@/features/application/userErrorSummary';
import type { AcfPacfResponse } from '@/features/application/stats/statsActions';
import { acfSeriesToBars, pacfSeriesToBars } from '@/shared/types/report';
import { useChartSeriesColors } from '@/shared/theme/chartTheme';
import { SectionHeader } from './RegressionShared';
import { InfoAccentButton } from './InfoViewControls';

const CorrelogramChart = React.lazy(() => import('@/views/PlotView/CorrelogramChart'));

export function ACFPACFBlock({ residuals, residualLabel }: { residuals?: number[]; residualLabel?: string }) {
  const { t } = useTranslation();
  const [lag, setLag] = useState(20);
  const [result, setResult] = useState<AcfPacfResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const seriesColors = useChartSeriesColors();

  const canRun = residuals != null && residuals.length >= 4 && lag >= 1 && lag <= 40;

  const handleRun = async () => {
    if (!canRun || !residuals) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await computeAcfPacf({ residuals, max_lag: lag });
      setResult(res);
    } catch (e) {
      setError(formatInlineUserError(e, t));
    } finally {
      setLoading(false);
    }
  };

  if (!residuals || residuals.length < 4) return null;

  const ciHalfWidth = 1.96 / Math.sqrt(residuals.length);

  return (
    <div className="mt-6">
      <SectionHeader
        title={residualLabel ? `ACF & PACF (检验对象: ${residualLabel})` : 'ACF & PACF (Correlogram)'}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2h2a2 2 0 002-2zm0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-border bg-card p-4 space-y-3">
        <div className="flex items-center gap-3">
          <Label className="text-[11px] text-muted-foreground uppercase tracking-wider">Lags</Label>
          <Input
            type="number"
            min={1}
            max={40}
            value={lag}
            onChange={(e) => setLag(Math.max(1, Math.min(40, parseInt(e.target.value, 10) || 1)))}
            className="w-20 font-mono text-sm"
          />
          <InfoAccentButton onClick={handleRun} disabled={!canRun} loading={loading}>
            生成
          </InfoAccentButton>
        </div>
        <div className="text-[10px] text-muted-foreground">
          Stata ac / pac 风格，95% 置信区间 ±1.96/√n
        </div>
        {error && <div className="text-xs text-red-400 font-mono">{error}</div>}
        {result && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-4">
            <div>
              <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
                <CorrelogramChart
                  data={acfSeriesToBars(result.acf)}
                  ciHalfWidth={ciHalfWidth}
                  title="ACF"
                />
              </Suspense>
            </div>
            <div>
              <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
                <CorrelogramChart
                  data={pacfSeriesToBars(result.pacf)}
                  ciHalfWidth={ciHalfWidth}
                  title="PACF"
                  color={seriesColors.secondary}
                />
              </Suspense>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

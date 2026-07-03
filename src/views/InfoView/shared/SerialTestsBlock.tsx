import React, { useState } from 'react';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { computeSerialTests } from '@/features/application/stats/statsActions';
import type { SerialTestsResponse } from '@/features/application/stats/statsActions';
import { SectionHeader } from './RegressionShared';
import { InfoAccentButton } from './InfoViewControls';
import { formatNum } from './utils';

export function SerialTestsBlock({ residuals, exog, residualLabel }: { residuals?: number[]; exog?: number[][]; residualLabel?: string }) {
  const [lag, setLag] = useState(20);
  const [bgDropMissing, setBgDropMissing] = useState(false);
  const [result, setResult] = useState<SerialTestsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const canRun = residuals != null && residuals.length >= 4 && lag >= 1 && lag <= 40;

  const handleRun = async () => {
    if (!canRun || !residuals) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await computeSerialTests({
        residuals,
        exog,
        lags: lag,
        bg_drop_missing: bgDropMissing,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  if (!residuals || residuals.length < 4) return null;

  return (
    <div className="mt-6">
      <SectionHeader
        title={residualLabel ? `Serial Correlation (检验对象: ${residualLabel})` : 'Serial Correlation (BG / Q / DW)'}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-border bg-card p-4 space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <Label className="text-[11px] text-muted-foreground uppercase tracking-wider">Lags (BG/Q)</Label>
          <Input
            type="number"
            min={1}
            max={40}
            value={lag}
            onChange={(e) => setLag(Math.max(1, Math.min(40, parseInt(e.target.value, 10) || 1)))}
            className="w-20 font-mono text-sm"
          />
          <div className="flex items-center gap-2">
            <Checkbox
              id="serial-bg-drop-missing"
              checked={bgDropMissing}
              onCheckedChange={(checked) => setBgDropMissing(checked === true)}
            />
            <Label htmlFor="serial-bg-drop-missing" className="cursor-pointer text-[11px] text-muted-foreground">
              BG: 去掉缺失值 (n-p)
            </Label>
          </div>
          <InfoAccentButton onClick={handleRun} disabled={!canRun} loading={loading}>
            生成
          </InfoAccentButton>
        </div>
        <div className="text-[10px] text-muted-foreground">
          BG: estat bgodfrey（勾选「去掉缺失值」= 不用 nomiss0）· Q: wntestq · DW: estat dwatson
        </div>
        {error && <div className="text-xs text-red-400 font-mono">{error}</div>}
        {result && (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 mt-4">
            {result.bg && (
              <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                <div className="text-[11px] text-muted-foreground font-mono mb-2">Breusch-Godfrey LM</div>
                <div className="text-foreground font-mono text-sm font-medium">
                  χ²({result.bg.lags}) = {formatNum(result.bg.stat)}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  p = {formatNum(result.bg.p_value)}
                  {result.bg.p_value < 0.05 ? (
                    <span className="text-amber-400 ml-1">*</span>
                  ) : null}
                </div>
              </div>
            )}
            {result.q && (
              <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                <div className="text-[11px] text-muted-foreground font-mono mb-2">Ljung-Box Q</div>
                <div className="text-foreground font-mono text-sm font-medium">
                  Q({result.q.lags}) = {formatNum(result.q.stat)}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  p = {formatNum(result.q.p_value)}
                  {result.q.p_value < 0.05 ? (
                    <span className="text-amber-400 ml-1">*</span>
                  ) : null}
                </div>
              </div>
            )}
            {result.dw != null && (
              <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                <div className="text-[11px] text-muted-foreground font-mono mb-2">Durbin-Watson</div>
                <div className="text-foreground font-mono text-sm font-medium">DW = {formatNum(result.dw)}</div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

import React, { useState } from 'react';
import { computeSerialTests } from '@/services/stats';
import type { SerialTestsResponse } from '@/services/stats';
import { SectionHeader } from './RegressionShared';
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
        lags: lag,
        exog: exog ?? undefined,
        bg_nomiss0: !bgDropMissing,
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
        title={residualLabel ? `序列相关检验 (检验对象: ${residualLabel})` : '序列相关检验'}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <label className="text-[11px] text-gray-500 uppercase tracking-wider">Lags (BG/Q)</label>
          <input
            type="number"
            min={1}
            max={40}
            value={lag}
            onChange={(e) => setLag(Math.max(1, Math.min(40, parseInt(e.target.value, 10) || 1)))}
            className="w-20 px-3 py-2 rounded-md bg-[#1a1d23] border border-gray-700/50 text-sm font-mono text-white focus:outline-none focus:border-[var(--accent-color)]/50"
          />
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={bgDropMissing}
              onChange={(e) => setBgDropMissing(e.target.checked)}
              className="rounded border-gray-600 bg-[#1a1d23] text-[var(--accent-color)] focus:ring-[var(--accent-color)]/50"
            />
            <span className="text-[11px] text-gray-400">BG: 去掉缺失值 (n-p)</span>
          </label>
          <button
            onClick={handleRun}
            disabled={!canRun || loading}
            className="px-4 py-2 rounded-md bg-[var(--accent-color)]/20 text-[var(--accent-color)] border border-[var(--accent-color)]/40 hover:bg-[var(--accent-color)]/30 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium transition-colors"
          >
            {loading ? '...' : '生成'}
          </button>
        </div>
        <div className="text-[10px] text-gray-500">
          BG: estat bgodfrey（勾选「去掉缺失值」= 不用 nomiss0）· Q: wntestq · DW: estat dwatson
        </div>
        {error && <div className="text-xs text-red-400 font-mono">{error}</div>}
        {result && (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 mt-4">
            {result.bg && (
              <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                <div className="text-[11px] text-gray-500 font-mono mb-2">Breusch-Godfrey LM</div>
                <div className="text-white font-mono text-sm font-medium">
                  χ²({result.bg.lags}) = {formatNum(result.bg.stat)}
                </div>
                <div className="text-xs text-gray-400 mt-1">
                  p = {formatNum(result.bg.p_value)}
                  {result.bg.p_value < 0.05 ? (
                    <span className="text-amber-400 ml-1">*</span>
                  ) : null}
                </div>
              </div>
            )}
            {result.q && (
              <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                <div className="text-[11px] text-gray-500 font-mono mb-2">Ljung-Box Q</div>
                <div className="text-white font-mono text-sm font-medium">
                  Q({result.q.lags}) = {formatNum(result.q.stat)}
                </div>
                <div className="text-xs text-gray-400 mt-1">
                  p = {formatNum(result.q.p_value)}
                  {result.q.p_value < 0.05 ? (
                    <span className="text-amber-400 ml-1">*</span>
                  ) : null}
                </div>
              </div>
            )}
            <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
              <div className="text-[11px] text-gray-500 font-mono mb-2">Durbin-Watson</div>
              <div className="text-white font-mono text-sm font-medium">
                d = {formatNum(result.dw.d)}
              </div>
              <div className="text-xs text-gray-400 mt-1">estat dwatson</div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

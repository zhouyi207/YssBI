import React, { useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { hypothesisTest } from '@/services/stats';
import type { HypothesisTestResponse } from '@/services/stats';
import { SectionHeader, formatNum } from './RegressionShared';
import { buildParamNames, linearFormToLatex, renderHypothesisLatex } from './utils';
import type { RegressionResultData } from './types';

function HypothesisFormulas({
  form,
  paramNames,
  className = '',
}: {
  form: string;
  paramNames: string[];
  className?: string;
}) {
  const parts = form.split(' ; ').map((s) => s.trim()).filter(Boolean);
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {parts.map((part, i) => {
        const html = renderHypothesisLatex(linearFormToLatex(part, paramNames));
        return (
          <div
            key={i}
            className="[&_.katex]:text-gray-200 [&_.katex]:text-xs [&_.katex]:block"
            dangerouslySetInnerHTML={{ __html: html ?? part }}
          />
        );
      })}
    </div>
  );
}

export function HypothesisTestBlock({ data }: { data: RegressionResultData }) {
  const [hypothesis, setHypothesis] = useState('');
  const [result, setResult] = useState<HypothesisTestResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const paramNames = useMemo(() => buildParamNames(data.coefficients), [data.coefficients]);
  const canRun =
    data.betas != null &&
    data.cov_beta != null &&
    data.model_basic_info.df_residual != null &&
    hypothesis.trim().length > 0;

  const handleRun = async () => {
    if (!canRun || !data.betas || !data.cov_beta) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await hypothesisTest({
        betas: data.betas,
        cov_beta: data.cov_beta,
        df_residual: data.model_basic_info.df_residual,
        param_names: paramNames,
        hypothesis: hypothesis.trim(),
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="mt-6">
      <SectionHeader
        title="Hypothesis Test (t / Wald)"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-3">
        <div className="flex gap-2">
          <input
            type="text"
            value={hypothesis}
            onChange={(e) => setHypothesis(e.target.value)}
            placeholder="e.g. x1 = 0 或 petal_width = -0.5626, petal_length = 0.7（逗号分隔多约束）"
            className="flex-1 px-3 py-2 rounded-md bg-[#1a1d23] border border-gray-700/50 text-sm font-mono text-white placeholder-gray-500 focus:outline-none focus:border-[var(--accent-color)]/50"
            onKeyDown={(e) => e.key === 'Enter' && handleRun()}
          />
          <button
            onClick={handleRun}
            disabled={!canRun || loading}
            className="px-4 py-2 rounded-md bg-[var(--accent-color)]/20 text-[var(--accent-color)] border border-[var(--accent-color)]/40 hover:bg-[var(--accent-color)]/30 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium transition-colors"
          >
            {loading ? '...' : 'Run'}
          </button>
        </div>
        <div className="text-[10px] text-gray-500">
          Param names: {paramNames.join(', ')}
        </div>
        {error && (
          <div className="text-xs text-red-400 font-mono">{error}</div>
        )}
        {result && (
          <div className="rounded-md bg-[#1a1d23] border border-gray-800/50 overflow-hidden">
            <div className="grid grid-cols-2 divide-x divide-gray-800/50">
              <div className="p-4 min-w-0">
                <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">H₀ 原假设</div>
                <HypothesisFormulas form={result.h0_form} paramNames={paramNames} />
              </div>
              <div className="p-4 min-w-0">
                <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">H₁ 备择假设</div>
                <HypothesisFormulas form={result.h1_form} paramNames={paramNames} />
              </div>
            </div>
            <div className="border-t border-gray-800/50 px-4 py-3 space-y-1.5 text-xs">
              <div className="flex justify-between">
                <span className="text-gray-500">
                  {result.test_type === "t" ? "t-statistic" : "F-statistic"}
                </span>
                <span className="font-mono text-white">{formatNum(result.stat, 4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500">df</span>
                <span className="font-mono text-gray-400">{result.df1}, {result.df2}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500">p-value</span>
                <span className={`font-mono font-medium ${result.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}`}>
                  {formatNum(result.p_value, 4)}
                </span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

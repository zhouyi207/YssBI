import React, { useMemo, useState, useCallback } from 'react';
import { PanelDidService } from '@/services/stats';
import {
  SectionHeader,
  PanelFESummaryGrid,
  ModelSummaryGrid,
  CoefficientsBlock,
  HypothesisTestBlock,
  DidEventStudyChart,
} from './shared';
import type { PanelDidResultData, OLSResultData, DidPlaceboFakeGroupBlock } from './shared/types';

export type { PanelDidResultData } from './shared/types';

export const DIDComponent: React.FC<{ data: PanelDidResultData }> = ({ data }) => {
  const {
    title,
    endog_name,
    treat_name,
    post_name,
    fe_twoway,
    error,
    parallel_trends,
    placebo,
    fake_group_engine,
    placebo_fake_group,
  } = data;

  const [permReps, setPermReps] = useState(399);
  const [rngSeed, setRngSeed] = useState(42);
  const [fakeGroupRi, setFakeGroupRi] = useState<DidPlaceboFakeGroupBlock | null>(null);
  const [fgLoading, setFgLoading] = useState(false);
  const [fgErr, setFgErr] = useState<string | null>(null);

  const fakeGroupDisplay = fakeGroupRi ?? placebo_fake_group ?? null;

  const runFakeGroupRi = useCallback(async () => {
    if (!fake_group_engine) return;
    setFgErr(null);
    setFgLoading(true);
    try {
      const n_perm = Math.max(1, Math.min(2000, Math.floor(permReps) || 399));
      const res = await PanelDidService.computeFakeGroupRi<typeof fake_group_engine & { n_perm: number; rng_seed: number }, DidPlaceboFakeGroupBlock>({
        ...fake_group_engine,
        n_perm,
        rng_seed: Number.isFinite(rngSeed) ? Math.max(0, Math.floor(Number(rngSeed))) : 42,
      });
      setFakeGroupRi(res);
    } catch (e) {
      setFgErr(e instanceof Error ? e.message : String(e));
    } finally {
      setFgLoading(false);
    }
  }, [fake_group_engine, permReps, rngSeed]);

  const didLabel = `${treat_name}×${post_name}`;
  const didRow = useMemo(() => {
    if (!fe_twoway?.coefficients) return null;
    return fe_twoway.coefficients.find((c) => c.variable === didLabel && c.category == null) ?? null;
  }, [fe_twoway, didLabel]);

  const hasCategorical = useMemo(
    () => (fe_twoway ? fe_twoway.coefficients.some((c) => c.category != null) : false),
    [fe_twoway]
  );

  if (error) {
    return (
      <div className="p-6 max-w-[900px] mx-auto">
        <h1 className="text-xl font-bold text-white mb-2">{title}</h1>
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-200">{error}</div>
      </div>
    );
  }

  if (!fe_twoway) {
    return (
      <div className="p-6 max-w-[900px] mx-auto">
        <h1 className="text-xl font-bold text-white mb-2">{title}</h1>
        <p className="text-sm text-gray-500">No two-way FE result.</p>
      </div>
    );
  }

  const ols = fe_twoway as OLSResultData;

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{title}</h1>
        <p className="text-xs text-gray-500 leading-relaxed">
          Outcome: <span className="text-gray-400">{endog_name}</span>
          {' · '}
          Treat: <span className="text-gray-400">{treat_name}</span>
          {' · '}
          Post: <span className="text-gray-400">{post_name}</span>
          {' · '}
          Two-way FE (entity + time), VCE 与 Panel Summary Config 一致（默认按个体聚类）
        </p>
      </div>

      <div className="mb-6 rounded-lg border border-indigo-500/25 bg-indigo-500/5 p-4 text-sm text-gray-300 leading-relaxed">
        <div className="font-medium text-indigo-300 mb-1">单期 DID（2×2）</div>
        <p>
          回归在 Y 上对可选控制变量与交互项{' '}
          <code className="text-emerald-400/90">{didLabel}</code> 做双向固定效应（与 Stata{' '}
          <code className="text-gray-500">reghdfe Y X i.treat#i.post, absorb(id t)</code> 同类）。Treat/Post
          主效应由 FE 吸收，仅报告 Treat×Post。平行趋势为事件研究前导项联合 Wald；安慰剂为政策前 H 期伪窗口×处理组。
        </p>
        {didRow && (
          <p className="mt-2 text-gray-400">
            点估计（{didLabel}）:{' '}
            <span className="text-white font-mono tabular-nums">{didRow.coef.toFixed(6)}</span>
            {' · '}
            p ={' '}
            <span className="text-white font-mono tabular-nums">{didRow.p_value.toFixed(4)}</span>
          </p>
        )}
      </div>

      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
            />
          </svg>
        }
      />
      {fe_twoway.diagnostic_info?.panel_fe_info ? (
        <PanelFESummaryGrid info={fe_twoway.model_basic_info} panelFe={fe_twoway.diagnostic_info.panel_fe_info} />
      ) : (
        <ModelSummaryGrid info={fe_twoway.model_basic_info} />
      )}

      <CoefficientsBlock
        coefficients={fe_twoway.coefficients}
        hasCategorical={hasCategorical}
        useZStat={
          fe_twoway.model_basic_info?.wald_chi2 != null || fe_twoway.model_basic_info?.lr_chi2 != null
        }
      />

      {fe_twoway.diagnostic_info?.omit_info && fe_twoway.diagnostic_info.omit_info.omitted.length > 0 && (
        <div className="mb-6 rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
          <div className="font-medium text-amber-400 mb-1">Omitted variables (collinearity)</div>
          <ul className="mt-2 space-y-1 text-sm font-mono text-gray-400">
            {fe_twoway.diagnostic_info.omit_info.omitted.map((o, i) => (
              <li key={i}>
                {o.variable}
                {o.category != null ? (
                  <span className="text-indigo-300 border border-indigo-500/25 rounded px-1.5 py-0.5 ml-1">
                    {o.category}
                  </span>
                ) : null}
                <span className="text-gray-500 text-xs ml-1">({o.reason})</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <HypothesisTestBlock data={ols} />

      {parallel_trends ? (
        <div className="mb-6 rounded-lg border border-cyan-500/25 bg-cyan-500/5 p-4 text-sm text-gray-300">
          <div className="font-medium text-cyan-300 mb-2">平行趋势检验（事件研究 / Wald）</div>
          {parallel_trends.available &&
          parallel_trends.chi2 != null &&
          parallel_trends.df != null &&
          parallel_trends.p_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                Wald χ² = <span className="text-white font-mono">{parallel_trends.chi2.toFixed(4)}</span>
                {' · '}
                df = <span className="text-white font-mono">{parallel_trends.df}</span>
                {' · '}
                p = <span className="text-white font-mono">{parallel_trends.p_value.toFixed(4)}</span>
              </p>
              {parallel_trends.reference_rel != null ? (
                <p className="text-gray-400 text-xs">
                  参照期 rel_time = {parallel_trends.reference_rel}（省略）；检验前导项：
                  {(parallel_trends.tested_rel_periods ?? []).join(', ') || '—'}
                </p>
              ) : null}
              <p className="text-gray-500 text-xs leading-relaxed">{parallel_trends.method_note}</p>
            </div>
          ) : (
            <p className="text-amber-200/90 text-xs leading-relaxed">{parallel_trends.method_note}</p>
          )}
          {(parallel_trends.event_study?.length ?? 0) > 0 ? (
            <>
              <div className="text-xs text-gray-500 mt-3 mb-1">事件研究系数（相对政策时点 × 处理组）</div>
              <DidEventStudyChart points={parallel_trends.event_study!} treatLabel={treat_name} />
            </>
          ) : null}
        </div>
      ) : null}

      {placebo ? (
        <div className="mb-6 rounded-lg border border-violet-500/25 bg-violet-500/5 p-4 text-sm text-gray-300">
          <div className="font-medium text-violet-300 mb-2">安慰剂 ① 虚构政策时点（政策前 H 期 × 真实处理组）</div>
          {placebo.available &&
          placebo.coef != null &&
          placebo.p_value != null &&
          placebo.std_err != null &&
          placebo.t_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                H = {placebo.horizon} 期伪窗口 · coef ={' '}
                <span className="text-white font-mono">{placebo.coef.toFixed(6)}</span>
                {' · '}
                se = <span className="text-white font-mono">{placebo.std_err.toFixed(6)}</span>
                {' · '}
                t = <span className="text-white font-mono">{placebo.t_value.toFixed(4)}</span>
                {' · '}
                p = <span className="text-white font-mono">{placebo.p_value.toFixed(4)}</span>
              </p>
              <p className="text-gray-500 text-xs leading-relaxed">{placebo.method_note}</p>
            </div>
          ) : (
            <p className="text-amber-200/90 text-xs leading-relaxed">{placebo.method_note}</p>
          )}
        </div>
      ) : null}

      {fake_group_engine || fakeGroupDisplay ? (
        <div className="mb-6 rounded-lg border border-fuchsia-500/25 bg-fuchsia-500/5 p-4 text-sm text-gray-300">
          <div className="font-medium text-fuchsia-300 mb-2">安慰剂 ② 虚构处理组（实体级随机置换）</div>
          {fake_group_engine ? (
            <div className="mb-4 flex flex-wrap items-end gap-3 text-xs">
              <label className="flex flex-col gap-1 text-gray-400">
                模拟次数（1–2000）
                <input
                  type="number"
                  min={1}
                  max={2000}
                  value={permReps}
                  onChange={(ev) => setPermReps(Number(ev.target.value))}
                  className="w-28 rounded border border-fuchsia-500/30 bg-black/30 px-2 py-1 text-white font-mono tabular-nums"
                />
              </label>
              <label className="flex flex-col gap-1 text-gray-400">
                随机种子
                <input
                  type="number"
                  value={rngSeed}
                  onChange={(ev) => setRngSeed(Number(ev.target.value))}
                  className="w-28 rounded border border-fuchsia-500/30 bg-black/30 px-2 py-1 text-white font-mono tabular-nums"
                />
              </label>
              <button
                type="button"
                disabled={fgLoading}
                onClick={() => void runFakeGroupRi()}
                className="rounded border border-fuchsia-400/50 bg-fuchsia-500/20 px-3 py-1.5 text-fuchsia-100 hover:bg-fuchsia-500/30 disabled:opacity-50"
              >
                {fgLoading ? '计算中…' : '计算置换检验'}
              </button>
            </div>
          ) : null}
          {fgErr ? (
            <p className="mb-2 text-red-300/90 text-xs leading-relaxed">{fgErr}</p>
          ) : null}
          {fakeGroupDisplay ? (
            fakeGroupDisplay.available &&
            fakeGroupDisplay.p_value_ri != null &&
            fakeGroupDisplay.observed_coef != null &&
            fakeGroupDisplay.perm_coef_mean != null &&
            fakeGroupDisplay.perm_coef_std != null ? (
              <div className="space-y-2">
                <p className="tabular-nums">
                  置换次数（成功拟合）{' '}
                  <span className="text-white font-mono">
                    {fakeGroupDisplay.n_perm_valid}/{fakeGroupDisplay.n_perm}
                  </span>
                  {' · '}
                  主回归 coef_obs ={' '}
                  <span className="text-white font-mono">{fakeGroupDisplay.observed_coef.toFixed(6)}</span>
                </p>
                <p className="tabular-nums">
                  置换系数 mean ={' '}
                  <span className="text-white font-mono">{fakeGroupDisplay.perm_coef_mean.toFixed(6)}</span>
                  {' · '}
                  sd ={' '}
                  <span className="text-white font-mono">{fakeGroupDisplay.perm_coef_std.toFixed(6)}</span>
                  {' · '}
                  RI p（双侧）={' '}
                  <span className="text-white font-mono">{fakeGroupDisplay.p_value_ri.toFixed(4)}</span>
                </p>
                <p className="text-gray-500 text-xs leading-relaxed">{fakeGroupDisplay.method_note}</p>
              </div>
            ) : (
              <p className="text-amber-200/90 text-xs leading-relaxed">{fakeGroupDisplay.method_note}</p>
            )
          ) : fake_group_engine ? (
            <p className="text-gray-500 text-xs leading-relaxed">
              默认不在图执行时做大量置换；确认样本量后在此设置次数并点击「计算置换检验」。
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
};

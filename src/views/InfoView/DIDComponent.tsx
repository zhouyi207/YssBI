import type { FC } from 'react';
import { useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useDidFakeGroupRi } from '@/features/application/stats/statsActions';
import {
  ReportLayout,
  ReportSection,
  PanelFESummaryGrid,
  ModelSummaryGrid,
  CoefficientsBlock,
  HypothesisTestBlock,
  DidEventStudyChart,
  OmittedVariablesAlert,
} from './shared';
import type { PanelDidResultData, OLSResultData } from './shared/types';

export type { PanelDidResultData } from './shared/types';

export const DIDComponent: FC<{ data: PanelDidResultData }> = ({ data }) => {
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

  const fakeGroup = useDidFakeGroupRi(fake_group_engine, placebo_fake_group);

  const didLabel = `${treat_name}×${post_name}`;
  const didRow = useMemo(() => {
    if (!fe_twoway?.coefficients) return null;
    return fe_twoway.coefficients.find((c) => c.variable === didLabel && c.category == null) ?? null;
  }, [fe_twoway, didLabel]);

  const hasCategorical = useMemo(
    () => (fe_twoway ? fe_twoway.coefficients.some((c) => c.category != null) : false),
    [fe_twoway],
  );

  if (error) {
    return (
      <ReportLayout title={title}>
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-200">{error}</div>
      </ReportLayout>
    );
  }

  if (!fe_twoway) {
    return (
      <ReportLayout title={title}>
        <p className="text-sm text-muted-foreground">No two-way FE result.</p>
      </ReportLayout>
    );
  }

  const ols = fe_twoway as OLSResultData;

  return (
    <ReportLayout
      title={title}
      subtitle={
        <p className="text-xs leading-relaxed text-muted-foreground">
          Outcome: <span className="text-muted-foreground">{endog_name}</span>
          {' · '}
          Treat: <span className="text-muted-foreground">{treat_name}</span>
          {' · '}
          Post: <span className="text-muted-foreground">{post_name}</span>
          {' · '}
          Two-way FE (entity + time), VCE 与 Panel Summary Config 一致（默认按个体聚类）
        </p>
      }
    >
      <div className="mb-6 rounded-lg border border-indigo-500/25 bg-indigo-500/5 p-4 text-sm leading-relaxed text-foreground">
        <div className="mb-1 font-medium text-indigo-300">单期 DID（2×2）</div>
        <p>
          回归在 Y 上对可选控制变量与交互项 <code className="text-emerald-400/90">{didLabel}</code> 做双向固定效应（与 Stata{' '}
          <code className="text-muted-foreground">reghdfe Y X i.treat#i.post, absorb(id t)</code> 同类）。Treat/Post
          主效应由 FE 吸收，仅报告 Treat×Post。平行趋势为事件研究前导项联合 Wald；安慰剂为政策前 H 期伪窗口×处理组。
        </p>
        {didRow && (
          <p className="mt-2 text-muted-foreground">
            点估计（{didLabel}）:{' '}
            <span className="font-mono tabular-nums text-foreground">{didRow.coef.toFixed(6)}</span>
            {' · '}
            p ={' '}
            <span className="font-mono tabular-nums text-foreground">
              {(didRow.p_value ?? 0).toFixed(4)}
            </span>
          </p>
        )}
      </div>

      <ReportSection title="Model Summary" icon="modelSummary">
        {fe_twoway.diagnostic_info?.panel_fe_info ? (
          <PanelFESummaryGrid info={fe_twoway.model_basic_info} panelFe={fe_twoway.diagnostic_info.panel_fe_info} />
        ) : (
          <ModelSummaryGrid info={fe_twoway.model_basic_info} />
        )}
      </ReportSection>

      <CoefficientsBlock
        coefficients={fe_twoway.coefficients}
        hasCategorical={hasCategorical}
        useZStat={fe_twoway.model_basic_info?.wald_chi2 != null || fe_twoway.model_basic_info?.lr_chi2 != null}
      />

      {fe_twoway.diagnostic_info ? <OmittedVariablesAlert diag={fe_twoway.diagnostic_info} /> : null}

      <HypothesisTestBlock data={ols} />

      {parallel_trends ? (
        <div className="mb-6 rounded-lg border border-cyan-500/25 bg-cyan-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-cyan-300">平行趋势检验（事件研究 / Wald）</div>
          {parallel_trends.available &&
          parallel_trends.chi2 != null &&
          parallel_trends.df != null &&
          parallel_trends.p_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                Wald χ² = <span className="font-mono text-foreground">{parallel_trends.chi2.toFixed(4)}</span>
                {' · '}
                df = <span className="font-mono text-foreground">{parallel_trends.df}</span>
                {' · '}
                p = <span className="font-mono text-foreground">{parallel_trends.p_value.toFixed(4)}</span>
              </p>
              {parallel_trends.reference_rel != null ? (
                <p className="text-xs text-muted-foreground">
                  参照期 rel_time = {parallel_trends.reference_rel}（省略）；检验前导项：
                  {(parallel_trends.tested_rel_periods ?? []).join(', ') || '—'}
                </p>
              ) : null}
              <p className="text-xs leading-relaxed text-muted-foreground">{parallel_trends.method_note}</p>
            </div>
          ) : (
            <p className="text-xs leading-relaxed text-amber-200/90">{parallel_trends.method_note}</p>
          )}
          {(parallel_trends.event_study?.length ?? 0) > 0 ? (
            <>
              <div className="mb-1 mt-3 text-xs text-muted-foreground">事件研究系数（相对政策时点 × 处理组）</div>
              <DidEventStudyChart points={parallel_trends.event_study!} treatLabel={treat_name} />
            </>
          ) : null}
        </div>
      ) : null}

      {placebo ? (
        <div className="mb-6 rounded-lg border border-violet-500/25 bg-violet-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-violet-300">安慰剂 ① 虚构政策时点（政策前 H 期 × 真实处理组）</div>
          {placebo.available &&
          placebo.coef != null &&
          placebo.p_value != null &&
          placebo.std_err != null &&
          placebo.t_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                H = {placebo.horizon} 期伪窗口 · coef ={' '}
                <span className="font-mono text-foreground">{placebo.coef.toFixed(6)}</span>
                {' · '}
                se = <span className="font-mono text-foreground">{placebo.std_err.toFixed(6)}</span>
                {' · '}
                t = <span className="font-mono text-foreground">{placebo.t_value.toFixed(4)}</span>
                {' · '}
                p = <span className="font-mono text-foreground">{placebo.p_value.toFixed(4)}</span>
              </p>
              <p className="text-xs leading-relaxed text-muted-foreground">{placebo.method_note}</p>
            </div>
          ) : (
            <p className="text-xs leading-relaxed text-amber-200/90">{placebo.method_note}</p>
          )}
        </div>
      ) : null}

      {fake_group_engine || fakeGroup.display ? (
        <div className="mb-6 rounded-lg border border-fuchsia-500/25 bg-fuchsia-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-fuchsia-300">安慰剂 ② 虚构处理组（实体级随机置换）</div>
          {fake_group_engine ? (
            <div className="mb-4 flex flex-wrap items-end gap-3 text-xs">
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="did-perm-reps" className="text-muted-foreground">
                  模拟次数（1–2000）
                </Label>
                <Input
                  id="did-perm-reps"
                  type="number"
                  min={1}
                  max={2000}
                  value={fakeGroup.permReps}
                  onChange={(ev) => fakeGroup.setPermReps(Number(ev.target.value))}
                  className="h-8 w-28 border-fuchsia-500/30 bg-muted/50 font-mono tabular-nums"
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="did-rng-seed" className="text-muted-foreground">
                  随机种子
                </Label>
                <Input
                  id="did-rng-seed"
                  type="number"
                  value={fakeGroup.rngSeed}
                  onChange={(ev) => fakeGroup.setRngSeed(Number(ev.target.value))}
                  className="h-8 w-28 border-fuchsia-500/30 bg-muted/50 font-mono tabular-nums"
                />
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={fakeGroup.loading}
                onClick={() => void fakeGroup.run()}
                className="border-fuchsia-400/50 bg-fuchsia-500/20 text-fuchsia-100 hover:bg-fuchsia-500/30"
              >
                {fakeGroup.loading ? '计算中…' : '计算置换检验'}
              </Button>
            </div>
          ) : null}
          {fakeGroup.error ? (
            <p className="mb-2 text-xs leading-relaxed text-red-300/90">{fakeGroup.error}</p>
          ) : null}
          {fakeGroup.display ? (
            fakeGroup.display.available &&
            fakeGroup.display.p_value_ri != null &&
            fakeGroup.display.observed_coef != null &&
            fakeGroup.display.perm_coef_mean != null &&
            fakeGroup.display.perm_coef_std != null ? (
              <div className="space-y-2">
                <p className="tabular-nums">
                  置换次数（成功拟合）{' '}
                  <span className="font-mono text-foreground">
                    {fakeGroup.display.n_perm_valid}/{fakeGroup.display.n_perm}
                  </span>
                  {' · '}
                  主回归 coef_obs ={' '}
                  <span className="font-mono text-foreground">{fakeGroup.display.observed_coef.toFixed(6)}</span>
                </p>
                <p className="tabular-nums">
                  置换系数 mean ={' '}
                  <span className="font-mono text-foreground">{fakeGroup.display.perm_coef_mean.toFixed(6)}</span>
                  {' · '}
                  sd = <span className="font-mono text-foreground">{fakeGroup.display.perm_coef_std.toFixed(6)}</span>
                  {' · '}
                  RI p（双侧）={' '}
                  <span className="font-mono text-foreground">{fakeGroup.display.p_value_ri.toFixed(4)}</span>
                </p>
                <p className="text-xs leading-relaxed text-muted-foreground">{fakeGroup.display.method_note}</p>
              </div>
            ) : (
              <p className="text-xs leading-relaxed text-amber-200/90">{fakeGroup.display.method_note}</p>
            )
          ) : fake_group_engine ? (
            <p className="text-xs leading-relaxed text-muted-foreground">
              默认不在图执行时做大量置换；确认样本量后在此设置次数并点击「计算置换检验」。
            </p>
          ) : null}
        </div>
      ) : null}
    </ReportLayout>
  );
};

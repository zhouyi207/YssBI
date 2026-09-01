import type { FC } from "react";
import { useMemo } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useDidFakeGroupRi } from "@/features/application/stats/statsActions";
import {
  ReportLayout,
  ReportSection,
  PanelFESummaryGrid,
  ModelSummaryGrid,
  CoefficientsBlock,
  HypothesisTestBlock,
  OmittedVariablesAlert,
  formatNum,
  formatNullableNum,
} from "./shared";
import { DidEventStudyChart } from "@/shared/charts/statistical";
import type {
  DidPlaceboFakeGroupUnavailableBlock,
  PanelDidResultData,
  OLSResultData,
} from "@/shared/types/report";

const DID_FAKE_GROUP_ERROR_KEYS: Readonly<Record<string, string>> = {
  internal_error: "did.fakeGroup.errors.internal_error",
  ipc_transport_failure: "did.fakeGroup.errors.ipc_transport_failure",
  ipc_malformed_error: "did.fakeGroup.errors.ipc_malformed_error",
  did_fake_group_request_failed: "did.fakeGroup.errors.did_fake_group_request_failed",
  did_fake_group_invalid_response: "did.fakeGroup.errors.did_fake_group_invalid_response",
  did_fake_group_invalid_initial_result:
    "did.fakeGroup.errors.did_fake_group_invalid_initial_result",
};

function didFakeGroupErrorMessage(code: string, t: TFunction): string {
  const key = DID_FAKE_GROUP_ERROR_KEYS[code];
  return key ? t(key) : t("did.fakeGroup.errors.unknown", { code });
}

function didFakeGroupUnavailableMessage(
  block: DidPlaceboFakeGroupUnavailableBlock,
  t: TFunction,
): string {
  const values = {
    nPerm: block.n_perm,
    nPermValid: block.n_perm_valid,
    minValidPermutations: block.min_valid_permutations,
    nTreatedEntities: block.n_treated_entities,
    nEntities: block.n_entities,
  };
  switch (block.unavailableCode) {
    case "no_treated_entities":
      return t("did.fakeGroup.unavailable.no_treated_entities", values);
    case "all_entities_treated":
      return t("did.fakeGroup.unavailable.all_entities_treated", values);
    case "insufficient_valid_permutations":
      return t("did.fakeGroup.unavailable.insufficient_valid_permutations", values);
  }
}

export const DIDComponent: FC<{ data: PanelDidResultData }> = ({ data }) => {
  const { t } = useTranslation();
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
    return (
      fe_twoway.coefficients.find((c) => c.variable === didLabel && c.category == null) ?? null
    );
  }, [fe_twoway, didLabel]);

  const hasCategorical = useMemo(
    () => (fe_twoway ? fe_twoway.coefficients.some((c) => c.category != null) : false),
    [fe_twoway],
  );

  if (error) {
    return (
      <ReportLayout title={title}>
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-700 dark:text-red-200">
          {error}
        </div>
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
          {" · "}
          Treat: <span className="text-muted-foreground">{treat_name}</span>
          {" · "}
          Post: <span className="text-muted-foreground">{post_name}</span>
          {" · "}
          Two-way FE (entity + time), VCE 与 Panel Summary Config 一致（默认按个体聚类）
        </p>
      }
    >
      <div className="mb-6 rounded-lg border border-indigo-500/25 bg-indigo-500/5 p-4 text-sm leading-relaxed text-foreground">
        <div className="mb-1 font-medium text-indigo-700 dark:text-indigo-300">单期 DID（2×2）</div>
        <p>
          回归在 Y 上对可选控制变量与交互项 <code className="text-emerald-400/90">{didLabel}</code>{" "}
          做双向固定效应（与 Stata{" "}
          <code className="text-muted-foreground">reghdfe Y X i.treat#i.post, absorb(id t)</code>{" "}
          同类）。Treat/Post 主效应由 FE 吸收，仅报告 Treat×Post。平行趋势为事件研究前导项联合
          Wald；安慰剂为政策前 H 期伪窗口×处理组。
        </p>
        {didRow && (
          <p className="mt-2 text-muted-foreground">
            点估计（{didLabel}）:{" "}
            <span className="font-mono tabular-nums text-foreground">
              {formatNum(didRow.coef, 6)}
            </span>
            {" · "}
            p ={" "}
            <span className="font-mono tabular-nums text-foreground">
              {formatNullableNum(didRow.p_value)}
            </span>
          </p>
        )}
      </div>

      <ReportSection title="Model Summary" icon="modelSummary">
        {fe_twoway.diagnostic_info?.panel_fe_info ? (
          <PanelFESummaryGrid
            info={fe_twoway.model_basic_info}
            panelFe={fe_twoway.diagnostic_info.panel_fe_info}
          />
        ) : (
          <ModelSummaryGrid info={fe_twoway.model_basic_info} />
        )}
      </ReportSection>

      <CoefficientsBlock
        coefficients={fe_twoway.coefficients}
        hasCategorical={hasCategorical}
        useZStat={
          fe_twoway.model_basic_info?.wald_chi2 != null ||
          fe_twoway.model_basic_info?.lr_chi2 != null
        }
      />

      {fe_twoway.diagnostic_info ? (
        <OmittedVariablesAlert diag={fe_twoway.diagnostic_info} />
      ) : null}

      <HypothesisTestBlock data={ols} />

      {parallel_trends ? (
        <div className="mb-6 rounded-lg border border-cyan-500/25 bg-cyan-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-cyan-700 dark:text-cyan-300">
            平行趋势检验（事件研究 / Wald）
          </div>
          {parallel_trends.available &&
          parallel_trends.chi2 != null &&
          parallel_trends.df != null &&
          parallel_trends.p_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                Wald χ² ={" "}
                <span className="font-mono text-foreground">{formatNum(parallel_trends.chi2)}</span>
                {" · "}
                df = <span className="font-mono text-foreground">{parallel_trends.df}</span>
                {" · "}
                p ={" "}
                <span className="font-mono text-foreground">
                  {formatNum(parallel_trends.p_value)}
                </span>
              </p>
              {parallel_trends.reference_rel != null ? (
                <p className="text-xs text-muted-foreground">
                  参照期 rel_time = {parallel_trends.reference_rel}（省略）；检验前导项：
                  {(parallel_trends.tested_rel_periods ?? []).join(", ") || "—"}
                </p>
              ) : null}
              <p className="text-xs leading-relaxed text-muted-foreground">
                {parallel_trends.method_note}
              </p>
            </div>
          ) : (
            <p className="text-xs leading-relaxed text-amber-700/90 dark:text-amber-200/90">
              {parallel_trends.method_note}
            </p>
          )}
          {(parallel_trends.event_study?.length ?? 0) > 0 ? (
            <>
              <div className="mb-1 mt-3 text-xs text-muted-foreground">
                事件研究系数（相对政策时点 × 处理组）
              </div>
              <DidEventStudyChart
                points={parallel_trends.event_study!}
                xLabel="Relative time (rel_time)"
                yLabel={`Coefficient (× ${treat_name})`}
                ariaLabel="Event study coefficients"
              />
            </>
          ) : null}
        </div>
      ) : null}

      {placebo ? (
        <div className="mb-6 rounded-lg border border-violet-500/25 bg-violet-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-violet-700 dark:text-violet-300">
            安慰剂 ① 虚构政策时点（政策前 H 期 × 真实处理组）
          </div>
          {placebo.available &&
          placebo.coef != null &&
          placebo.p_value != null &&
          placebo.std_err != null &&
          placebo.t_value != null ? (
            <div className="space-y-2">
              <p className="tabular-nums">
                H = {placebo.horizon} 期伪窗口 · coef ={" "}
                <span className="font-mono text-foreground">{formatNum(placebo.coef, 6)}</span>
                {" · "}
                se ={" "}
                <span className="font-mono text-foreground">{formatNum(placebo.std_err, 6)}</span>
                {" · "}
                t = <span className="font-mono text-foreground">{formatNum(placebo.t_value)}</span>
                {" · "}
                p = <span className="font-mono text-foreground">{formatNum(placebo.p_value)}</span>
              </p>
              <p className="text-xs leading-relaxed text-muted-foreground">{placebo.method_note}</p>
            </div>
          ) : (
            <p className="text-xs leading-relaxed text-amber-700/90 dark:text-amber-200/90">
              {placebo.method_note}
            </p>
          )}
        </div>
      ) : null}

      {fake_group_engine || fakeGroup.display ? (
        <div className="mb-6 rounded-lg border border-fuchsia-500/25 bg-fuchsia-500/5 p-4 text-sm text-foreground">
          <div className="mb-2 font-medium text-fuchsia-700 dark:text-fuchsia-300">
            安慰剂 ② 虚构处理组（实体级随机置换）
          </div>
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
                {fakeGroup.loading ? "计算中…" : "计算置换检验"}
              </Button>
            </div>
          ) : null}
          {fakeGroup.error ? (
            <div
              role="alert"
              className="mb-2 space-y-1 text-xs leading-relaxed text-red-700/90 dark:text-red-300/90"
            >
              <p>
                <span className="font-mono">[{fakeGroup.error.code}]</span>{" "}
                {didFakeGroupErrorMessage(fakeGroup.error.code, t)}
              </p>
              {fakeGroup.error.incidentId ? (
                <p>
                  {t("common.incidentId")}:{" "}
                  <span className="font-mono">{fakeGroup.error.incidentId}</span>
                </p>
              ) : null}
            </div>
          ) : null}
          {fakeGroup.display ? (
            fakeGroup.display.available ? (
              <div className="space-y-2">
                <p className="tabular-nums">
                  置换次数（成功拟合）{" "}
                  <span className="font-mono text-foreground">
                    {fakeGroup.display.n_perm_valid}/{fakeGroup.display.n_perm}
                  </span>
                  {" · "}
                  主回归 coef_obs ={" "}
                  <span className="font-mono text-foreground">
                    {formatNum(fakeGroup.display.observed_coef, 6)}
                  </span>
                </p>
                <p className="tabular-nums">
                  置换系数 mean ={" "}
                  <span className="font-mono text-foreground">
                    {formatNum(fakeGroup.display.perm_coef_mean, 6)}
                  </span>
                  {" · "}
                  sd ={" "}
                  <span className="font-mono text-foreground">
                    {formatNum(fakeGroup.display.perm_coef_std, 6)}
                  </span>
                  {" · "}
                  RI p（双侧）={" "}
                  <span className="font-mono text-foreground">
                    {formatNum(fakeGroup.display.p_value_ri)}
                  </span>
                </p>
                <p className="text-xs leading-relaxed text-muted-foreground">
                  {t("did.fakeGroup.methodology", {
                    nPerm: fakeGroup.display.n_perm,
                    nPermValid: fakeGroup.display.n_perm_valid,
                    nTreatedEntities: fakeGroup.display.n_treated_entities,
                    nEntities: fakeGroup.display.n_entities,
                  })}
                </p>
              </div>
            ) : (
              <p className="text-xs leading-relaxed text-amber-700/90 dark:text-amber-200/90">
                {didFakeGroupUnavailableMessage(fakeGroup.display, t)}
              </p>
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

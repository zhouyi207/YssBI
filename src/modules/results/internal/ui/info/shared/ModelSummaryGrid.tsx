import { KeyValue } from "@/components/ui-presentation/KeyValue";
import type { UiMetric } from "@/shared/types/domain/uiData";
import type { LinearModelInfo } from "@/shared/types/report";

export function ModelSummaryGrid({
  info,
  executionTimeMs,
}: {
  info: LinearModelInfo;
  executionTimeMs?: number;
}) {
  const items: UiMetric[] = [
    { id: "model", label: "Model", value: info.model_type, format: "text" },
    { id: "method", label: "Method", value: info.method, format: "text" },
    { id: "rSquared", label: "R-squared", value: info.r_squared, format: "number" },
    { id: "adjRSquared", label: "Adj. R-squared", value: info.adj_r_squared, format: "number" },
  ];
  if (info.wald_chi2 != null) {
    items.push(
      {
        id: "statistic",
        label: `Wald chi2(${info.df_model})`,
        value: info.wald_chi2,
        format: "number",
      },
      {
        id: "pValue",
        label: "Prob > chi2",
        value: info.prob_wald_chi2 ?? "—",
        format: info.prob_wald_chi2 == null ? "text" : "pValue",
      },
    );
  } else {
    items.push(
      { id: "statistic", label: "F-statistic", value: info.f_statistic, format: "number" },
      { id: "pValue", label: "Prob (F-statistic)", value: info.prob_f_statistic, format: "pValue" },
    );
  }
  items.push(
    {
      id: "observations",
      label: "No. Observations",
      value: info.num_observation,
      format: "integer",
    },
    { id: "covariance", label: "Covariance Type", value: info.covariance_type, format: "text" },
    { id: "dfModel", label: "Df Model", value: info.df_model, format: "integer" },
    { id: "dfResidual", label: "Df Residual", value: info.df_residual, format: "integer" },
  );
  if (info.aic != null) items.push({ id: "aic", label: "AIC", value: info.aic, format: "number" });
  if (info.bic != null) items.push({ id: "bic", label: "BIC", value: info.bic, format: "number" });
  items.push({ id: "dfTotal", label: "Df Total", value: info.df_total, format: "integer" });
  if (executionTimeMs != null)
    items.push({
      id: "executionTime",
      label: "后端计算耗时",
      value: `${executionTimeMs} ms`,
      format: "text",
    });
  return <KeyValue items={items} />;
}

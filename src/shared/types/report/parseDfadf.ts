/**
 * DF / ADF 报告 IPC 窄化
 */

import { isFiniteNumber, isNonNegativeInteger, isRecord, isString } from "./guards";
import { parseObjectArray } from "./parseCommon";
import type { DFADFRegRowData, DFADFSummaryListResultData, DFADFSummaryResultData } from "./dfadf";

function parseDfAdfRegRow(raw: unknown): DFADFRegRowData | null {
  if (!isRecord(raw) || !isString(raw.variable)) return null;
  const nums = ["coef", "std_err", "t", "p_value", "ci_lower", "ci_upper"] as const;
  for (const key of nums) {
    if (!isFiniteNumber(raw[key])) return null;
  }
  return {
    variable: raw.variable,
    coef: raw.coef as number,
    std_err: raw.std_err as number,
    t: raw.t as number,
    p_value: raw.p_value as number,
    ci_lower: raw.ci_lower as number,
    ci_upper: raw.ci_upper as number,
  };
}

export function parseDfAdfSummaryResultData(raw: unknown): DFADFSummaryResultData | null {
  if (!isRecord(raw) || !isString(raw.title) || !isString(raw.var_name) || !isString(raw.h0))
    return null;
  if (!isFiniteNumber(raw.test_statistic) || !isFiniteNumber(raw.p_value)) return null;
  if (!isFiniteNumber(raw.critical_value_1pct) || !isFiniteNumber(raw.critical_value_5pct))
    return null;
  if (!isFiniteNumber(raw.critical_value_10pct)) return null;
  if (typeof raw.use_t_distribution !== "boolean" || !isNonNegativeInteger(raw.num_obs))
    return null;
  if (!isNonNegativeInteger(raw.lags) || !isString(raw.regression)) return null;
  if (!isFiniteNumber(raw.coef_lagged) || !isFiniteNumber(raw.std_err_lagged)) return null;
  const regression_table = parseObjectArray(raw.regression_table, parseDfAdfRegRow);
  if (!regression_table) return null;
  return {
    title: raw.title,
    var_name: raw.var_name,
    h0: raw.h0,
    test_statistic: raw.test_statistic,
    critical_value_1pct: raw.critical_value_1pct,
    critical_value_5pct: raw.critical_value_5pct,
    critical_value_10pct: raw.critical_value_10pct,
    p_value: raw.p_value,
    use_t_distribution: raw.use_t_distribution,
    num_obs: raw.num_obs,
    lags: raw.lags,
    regression: raw.regression,
    coef_lagged: raw.coef_lagged,
    std_err_lagged: raw.std_err_lagged,
    regression_table,
  };
}

export function parseDfAdfSummaryListResultData(raw: unknown): DFADFSummaryListResultData | null {
  if (!isRecord(raw) || !isString(raw.title) || !isString(raw.var_name)) return null;
  const items = parseObjectArray(raw.items, parseDfAdfSummaryResultData);
  if (!items || items.length === 0) return null;
  return { title: raw.title, var_name: raw.var_name, items };
}

/**
 * VEC / vecrank 报告 IPC 窄化
 */

import {
  assignPresentKeys,
  isFiniteNumber,
  isNonNegativeInteger,
  isRecord,
  isString,
} from "./guards";
import { parseStringArray, parseObjectArray } from "./parseCommon";
import { parseVarLmar, parseVarStableRow } from "./parseVar";
import type { VECSummaryResultData, VecRankResultData } from "./vec";

const VEC_SUMMARY_OPTIONAL_KEYS = [
  "beta_var_names",
  "beta_std_err",
  "beta_z_value",
  "beta_p_value",
  "beta_ci_lower",
  "beta_ci_upper",
] as const satisfies readonly (keyof VECSummaryResultData)[];

export function parseVecSummaryResultData(raw: unknown): VECSummaryResultData | null {
  if (!isRecord(raw) || !isString(raw.title)) return null;
  const var_names = parseStringArray(raw.var_names);
  if (!var_names || !isFiniteNumber(raw.num_observation) || !isNonNegativeInteger(raw.rank))
    return null;
  if (!isNonNegativeInteger(raw.lags) || !isString(raw.trend_spec)) return null;
  if (
    !Array.isArray(raw.equations) ||
    !Array.isArray(raw.coefficients) ||
    !Array.isArray(raw.beta)
  ) {
    return null;
  }
  if (!Array.isArray(raw.cointegrating_equations)) return null;
  if (
    !isFiniteNumber(raw.log_likelihood) ||
    !isFiniteNumber(raw.aic) ||
    !isFiniteNumber(raw.hqic) ||
    !isFiniteNumber(raw.sbic) ||
    !isFiniteNumber(raw.det_sigma_ml)
  ) {
    return null;
  }

  const veclmar =
    raw.veclmar === undefined
      ? undefined
      : (parseObjectArray(raw.veclmar, parseVarLmar) ?? undefined);
  if (raw.veclmar !== undefined && veclmar === undefined) return null;
  const vecstable =
    raw.vecstable === undefined
      ? undefined
      : (parseObjectArray(raw.vecstable, parseVarStableRow) ?? undefined);
  if (raw.vecstable !== undefined && vecstable === undefined) return null;

  return assignPresentKeys(
    {
      title: raw.title,
      var_names,
      num_observation: raw.num_observation,
      log_likelihood: raw.log_likelihood,
      aic: raw.aic,
      hqic: raw.hqic,
      sbic: raw.sbic,
      det_sigma_ml: raw.det_sigma_ml,
      rank: raw.rank,
      lags: raw.lags,
      trend_spec: raw.trend_spec,
      equations: raw.equations,
      coefficients: raw.coefficients,
      beta: raw.beta,
      cointegrating_equations: raw.cointegrating_equations,
      veclmar,
      vecstable,
    },
    raw,
    VEC_SUMMARY_OPTIONAL_KEYS,
  );
}

export function parseVecRankResultData(raw: unknown): VecRankResultData | null {
  if (!isRecord(raw) || !isString(raw.title) || !isString(raw.trend_spec)) return null;
  const var_names = parseStringArray(raw.var_names);
  if (!var_names || !isFiniteNumber(raw.num_observation) || !isNonNegativeInteger(raw.n_lags))
    return null;
  if (typeof raw.show_max_eigen !== "boolean" || !Array.isArray(raw.rows) || !isString(raw.note))
    return null;
  const rankFields = [
    "selected_rank_trace_95",
    "selected_rank_trace_99",
    "selected_rank_max_95",
    "selected_rank_max_99",
  ] as const;
  for (const key of rankFields) {
    if (!isNonNegativeInteger(raw[key])) return null;
  }
  return {
    kind: typeof raw.kind === "string" ? raw.kind : "vec_rank",
    title: raw.title,
    var_names,
    num_observation: raw.num_observation,
    n_lags: raw.n_lags,
    trend_spec: raw.trend_spec,
    show_max_eigen: raw.show_max_eigen,
    selected_rank_trace_95: raw.selected_rank_trace_95 as number,
    selected_rank_trace_99: raw.selected_rank_trace_99 as number,
    selected_rank_max_95: raw.selected_rank_max_95 as number,
    selected_rank_max_99: raw.selected_rank_max_99 as number,
    rows: raw.rows,
    note: raw.note,
  };
}

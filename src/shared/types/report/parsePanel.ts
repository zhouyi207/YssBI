/**
 * 面板 / DID 报告 IPC 窄化
 */

import { assignPresentKeys, isRecord, isString } from "./guards";
import { parseRegressionResultData } from "./parseRegression";
import type { OLSResultData } from "./regression";
import type { PanelSummaryResult } from "./panel";
import type { PanelDidResultData } from "./did";

const PANEL_MODEL_KEYS = [
  "mixed_ols",
  "fe",
  "fe_time",
  "fe_twoway",
  "lsdv",
  "lsdv_time",
  "lsdv_twoway",
  "fd",
  "re_fgls",
  "re_mle",
  "re_be",
  "re_fgls_time",
  "re_mle_time",
  "re_be_time",
  "re_fgls_twoway",
  "re_mle_twoway",
] as const;

const PANEL_SUMMARY_OPTIONAL_KEYS = [
  "selection_tests",
  "errors",
] as const satisfies readonly (keyof PanelSummaryResult)[];

const PANEL_DID_OPTIONAL_KEYS = [
  "parallel_trends",
  "placebo",
  "fake_group_engine",
  "placebo_fake_group",
] as const satisfies readonly (keyof PanelDidResultData)[];

function parsePanelNestedModels(raw: Record<string, unknown>): Partial<PanelSummaryResult> | null {
  const out: Partial<PanelSummaryResult> = {};
  for (const key of PANEL_MODEL_KEYS) {
    if (raw[key] === undefined) continue;
    const parsed = parseRegressionResultData(raw[key]);
    if (!parsed) return null;
    (out as Record<string, OLSResultData>)[key] = parsed;
  }
  return out;
}

export function parsePanelSummaryResult(raw: unknown): PanelSummaryResult | null {
  if (!isRecord(raw) || !isString(raw.title) || !isString(raw.endog_name)) return null;
  const nested = parsePanelNestedModels(raw);
  if (!nested) return null;
  const hasAnyModel = PANEL_MODEL_KEYS.some((key) => nested[key as keyof typeof nested] != null);
  if (!hasAnyModel && raw.selection_tests === undefined) return null;
  return assignPresentKeys(
    {
      title: raw.title,
      endog_name: raw.endog_name,
      ...nested,
    },
    raw,
    PANEL_SUMMARY_OPTIONAL_KEYS,
  );
}

export function parsePanelDidResultData(raw: unknown): PanelDidResultData | null {
  if (!isRecord(raw) || raw.kind !== "panel_did") return null;
  if (
    !isString(raw.title) ||
    !isString(raw.endog_name) ||
    !isString(raw.treat_name) ||
    !isString(raw.post_name)
  ) {
    return null;
  }
  let fe_twoway: OLSResultData | undefined;
  if (raw.fe_twoway !== undefined) {
    const parsed = parseRegressionResultData(raw.fe_twoway);
    if (!parsed) return null;
    fe_twoway = parsed;
  }
  return assignPresentKeys(
    {
      kind: "panel_did" as const,
      title: raw.title,
      endog_name: raw.endog_name,
      treat_name: raw.treat_name,
      post_name: raw.post_name,
      fe_twoway,
      error: typeof raw.error === "string" ? raw.error : undefined,
    },
    raw,
    PANEL_DID_OPTIONAL_KEYS,
  );
}

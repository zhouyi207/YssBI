/**
 * 面板 DID 报告 DTO
 */

import { isFiniteNumber, isRecord } from "./guards";
import type { OLSResultData } from "./regression";

export interface DidEventStudyPoint {
  rel_time: number;
  coef: number;
  std_err: number;
  ci_low: number;
  ci_high: number;
  is_reference?: boolean;
}

export interface DidParallelTrendsBlock {
  available: boolean;
  chi2?: number;
  df?: number;
  p_value?: number;
  reference_rel?: number;
  tested_rel_periods?: number[];
  event_study?: DidEventStudyPoint[];
  method_note: string;
}

export interface DidPlaceboTimingBlock {
  available: boolean;
  coef?: number;
  std_err?: number;
  t_value?: number;
  p_value?: number;
  horizon: number;
  method_note: string;
}

export type DidFakeGroupUnavailableCode =
  | "no_treated_entities"
  | "all_entities_treated"
  | "insufficient_valid_permutations";

interface DidPlaceboFakeGroupBase {
  n_perm: number;
  n_perm_valid: number;
  min_valid_permutations: number;
  n_entities: number;
  n_treated_entities: number;
}

export interface DidPlaceboFakeGroupAvailableBlock extends DidPlaceboFakeGroupBase {
  available: true;
  unavailableCode?: never;
  observed_coef: number;
  p_value_ri: number;
  perm_coef_mean: number;
  perm_coef_std: number;
}

export interface DidPlaceboFakeGroupUnavailableBlock extends DidPlaceboFakeGroupBase {
  available: false;
  unavailableCode: DidFakeGroupUnavailableCode;
  observed_coef?: never;
  p_value_ri?: never;
  perm_coef_mean?: never;
  perm_coef_std?: never;
}

export type DidPlaceboFakeGroupBlock =
  | DidPlaceboFakeGroupAvailableBlock
  | DidPlaceboFakeGroupUnavailableBlock;

const FAKE_GROUP_BASE_KEYS = [
  "available",
  "n_perm",
  "n_perm_valid",
  "min_valid_permutations",
  "n_entities",
  "n_treated_entities",
] as const;
const FAKE_GROUP_SUCCESS_KEYS = [
  ...FAKE_GROUP_BASE_KEYS,
  "observed_coef",
  "p_value_ri",
  "perm_coef_mean",
  "perm_coef_std",
] as const;
const FAKE_GROUP_UNAVAILABLE_KEYS = [...FAKE_GROUP_BASE_KEYS, "unavailableCode"] as const;
const FAKE_GROUP_UNAVAILABLE_CODES = new Set<DidFakeGroupUnavailableCode>([
  "no_treated_entities",
  "all_entities_treated",
  "insufficient_valid_permutations",
]);

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function hasExactKeys(raw: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(raw);
  return actual.length === keys.length && actual.every((key) => keys.includes(key));
}

function hasValidFakeGroupCounts(
  raw: Record<string, unknown>,
): raw is Record<string, unknown> & DidPlaceboFakeGroupBase {
  return (
    isNonNegativeSafeInteger(raw.n_perm) &&
    raw.n_perm > 0 &&
    raw.n_perm <= 2000 &&
    isNonNegativeSafeInteger(raw.n_perm_valid) &&
    raw.n_perm_valid <= raw.n_perm &&
    isNonNegativeSafeInteger(raw.min_valid_permutations) &&
    raw.min_valid_permutations > 0 &&
    isNonNegativeSafeInteger(raw.n_entities) &&
    raw.n_entities > 0 &&
    isNonNegativeSafeInteger(raw.n_treated_entities) &&
    raw.n_treated_entities <= raw.n_entities
  );
}

export function parseDidPlaceboFakeGroupBlock(raw: unknown): DidPlaceboFakeGroupBlock | null {
  if (!isRecord(raw) || !hasValidFakeGroupCounts(raw)) return null;
  if (raw.available === true) {
    if (!hasExactKeys(raw, FAKE_GROUP_SUCCESS_KEYS)) return null;
    if (raw.n_perm_valid < raw.min_valid_permutations) return null;
    if (raw.n_treated_entities === 0 || raw.n_treated_entities >= raw.n_entities) return null;
    if (
      !isFiniteNumber(raw.observed_coef) ||
      !isFiniteNumber(raw.p_value_ri) ||
      raw.p_value_ri < 0 ||
      raw.p_value_ri > 1 ||
      !isFiniteNumber(raw.perm_coef_mean) ||
      !isFiniteNumber(raw.perm_coef_std) ||
      raw.perm_coef_std < 0
    ) {
      return null;
    }
    return {
      available: true,
      observed_coef: raw.observed_coef,
      n_perm: raw.n_perm,
      n_perm_valid: raw.n_perm_valid,
      min_valid_permutations: raw.min_valid_permutations,
      n_entities: raw.n_entities,
      n_treated_entities: raw.n_treated_entities,
      p_value_ri: raw.p_value_ri,
      perm_coef_mean: raw.perm_coef_mean,
      perm_coef_std: raw.perm_coef_std,
    };
  }
  if (raw.available !== false || !hasExactKeys(raw, FAKE_GROUP_UNAVAILABLE_KEYS)) return null;
  if (
    typeof raw.unavailableCode !== "string" ||
    !FAKE_GROUP_UNAVAILABLE_CODES.has(raw.unavailableCode as DidFakeGroupUnavailableCode)
  ) {
    return null;
  }

  const unavailableCode = raw.unavailableCode as DidFakeGroupUnavailableCode;
  const expectedCounts =
    unavailableCode === "no_treated_entities"
      ? raw.n_treated_entities === 0 && raw.n_perm_valid === 0
      : unavailableCode === "all_entities_treated"
        ? raw.n_treated_entities === raw.n_entities && raw.n_perm_valid === 0
        : raw.n_treated_entities > 0 &&
          raw.n_treated_entities < raw.n_entities &&
          raw.n_perm_valid < raw.min_valid_permutations;
  if (!expectedCounts) return null;

  return {
    available: false,
    unavailableCode,
    n_perm: raw.n_perm,
    n_perm_valid: raw.n_perm_valid,
    min_valid_permutations: raw.min_valid_permutations,
    n_entities: raw.n_entities,
    n_treated_entities: raw.n_treated_entities,
  };
}

export interface ExogLabelEntry {
  variable: string;
  category?: string | null;
}

export interface DidFakeGroupEnginePayload {
  endog: number[];
  exog_row_major: number[];
  ncols: number;
  all_labels: ExogLabelEntry[];
  entity_id: number[];
  time_id: number[];
  post: number[];
  treat: number[];
  did_label: string;
  observed_coef: number;
  constant: boolean;
  cov_type: string;
}

export interface PanelDidResultData {
  kind: "panel_did";
  title: string;
  endog_name: string;
  treat_name: string;
  post_name: string;
  fe_twoway?: OLSResultData;
  error?: string;
  parallel_trends?: DidParallelTrendsBlock;
  placebo?: DidPlaceboTimingBlock;
  fake_group_engine?: DidFakeGroupEnginePayload | null;
  placebo_fake_group?: DidPlaceboFakeGroupBlock;
}

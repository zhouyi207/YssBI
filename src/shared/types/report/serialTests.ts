/**
 * 序列相关检验 DTO（对齐 Rust `command_serial_tests.rs`）
 * IPC 边界经 `normalizeSerialTestsResponse` 窄化。
 */

import { isFiniteNumber, isNonNegativeInteger, isRecord } from "./guards";

export interface SerialTestWithLagDTO {
  stat: number;
  p_value: number;
  lags: number;
}

/** Durbin-Watson 结果（`{ d: number }`，非裸 number） */
export interface DurbinWatsonResultDTO {
  d: number;
}

export interface SerialTestsResponseDTO {
  bg?: SerialTestWithLagDTO;
  q?: SerialTestWithLagDTO;
  dw: DurbinWatsonResultDTO;
}

export interface SerialTestsRequestDTO {
  residuals: number[];
  lags: number;
  exog?: number[][];
  bg_nomiss0?: boolean;
}

function normalizeSerialTestWithLag(raw: unknown): SerialTestWithLagDTO | undefined {
  if (!isRecord(raw)) return undefined;
  const stat = raw.stat;
  const p_value = raw.p_value;
  const lags = raw.lags;
  if (
    !isFiniteNumber(stat) ||
    !isFiniteNumber(p_value) ||
    !isNonNegativeInteger(lags) ||
    lags < 1
  ) {
    return undefined;
  }
  return { stat, p_value, lags };
}

export function normalizeDurbinWatsonResult(raw: unknown): DurbinWatsonResultDTO | null {
  if (!isRecord(raw) || !isFiniteNumber(raw.d)) return null;
  return { d: raw.d };
}

/** 窄化 `compute_serial_tests` 响应；拒绝 `dw` 为裸 number 等漂移形态。 */
export function normalizeSerialTestsResponse(raw: unknown): SerialTestsResponseDTO | null {
  if (!isRecord(raw)) return null;
  const dw = normalizeDurbinWatsonResult(raw.dw);
  if (!dw) return null;
  const bg = raw.bg === undefined ? undefined : normalizeSerialTestWithLag(raw.bg);
  if (raw.bg !== undefined && !bg) return null;
  const q = raw.q === undefined ? undefined : normalizeSerialTestWithLag(raw.q);
  if (raw.q !== undefined && !q) return null;
  return { bg, q, dw };
}

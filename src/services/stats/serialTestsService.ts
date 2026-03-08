import { invoke } from "@tauri-apps/api/core";

/** 序列相关检验请求（与后端 SerialTestsRequest 对应） */
export interface SerialTestsRequest {
  residuals: number[];
  lags: number;
  /** 回归设计矩阵 X（行优先），BG 检验需要 */
  exog?: number[][];
  /** BG: true=nomiss0(缺失用0填充)；false=去掉前p个观测 */
  bg_nomiss0?: boolean;
}

/** BG/Q 检验结果（需 lag） */
export interface SerialTestWithLag {
  stat: number;
  p_value: number;
  lags: number;
}

/** DW 检验结果（无需 lag） */
export interface DurbinWatsonResult {
  d: number;
}

/** 序列相关检验结果（与后端 SerialTestsResponse 对应） */
export interface SerialTestsResponse {
  bg?: SerialTestWithLag;
  q?: SerialTestWithLag;
  dw: DurbinWatsonResult;
}

/** 计算 BG、Q、DW 检验 */
export async function computeSerialTests(
  req: SerialTestsRequest
): Promise<SerialTestsResponse> {
  return await invoke<SerialTestsResponse>("compute_serial_tests", { req });
}

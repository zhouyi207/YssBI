import { invoke } from "@tauri-apps/api/core";

/** ACF/PACF 请求（与后端 AcfPacfRequest 对应） */
export interface AcfPacfRequest {
  residuals: number[];
  max_lag: number;
}

/** ACF/PACF 结果（与后端 AcfPacfResponse 对应） */
export interface AcfPacfResponse {
  /** ACF: lag 0..=max_lag，lag 0 恒为 1.0 */
  acf: number[];
  /** PACF: lag 1..=max_lag */
  pacf: number[];
  /** 样本量 */
  n: number;
}

/** 计算 ACF 和 PACF */
export async function computeAcfPacf(req: AcfPacfRequest): Promise<AcfPacfResponse> {
  return await invoke<AcfPacfResponse>("compute_acf_pacf", { req });
}

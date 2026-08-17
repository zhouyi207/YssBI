import { invokeCommand } from "@/services/ipc";

/** 假设检验请求（与后端 HypothesisTestRequest 对应） */
export interface HypothesisTestRequest {
  betas: number[];
  cov_beta: number[][];
  df_residual: number;
  param_names: string[];
  hypothesis: string;
}

/** 假设检验结果（与后端 HypothesisTestResponse 对应） */
export interface HypothesisTestResponse {
  test_type: "t" | "wald";
  /** 原假设 H0 的线性形式 */
  h0_form: string;
  /** 备择假设 H1 的线性形式 */
  h1_form: string;
  alternative: string;
  r_beta_minus_r: number;
  stat: number;
  df1: number;
  df2: number;
  p_value: number;
}

/**
 * 执行 Wald 假设检验
 * @param req 假设检验请求
 * @returns 检验结果
 */
export async function hypothesisTest(req: HypothesisTestRequest): Promise<HypothesisTestResponse> {
  return await invokeCommand<HypothesisTestResponse>("hypothesis_test", { req: req });
}

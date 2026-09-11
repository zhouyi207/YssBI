import { useState } from "react";
import { useTranslation } from "react-i18next";
import { hypothesisTest } from "@/features/application/stats/statsActions";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import type { HypothesisTestResponse } from "@/features/application/stats/statsActions";
import { buildParamNames } from "@/shared/stats/regressionReportUtils";
import type { RegressionResultData } from "@/shared/types/report";

export interface HypothesisTestSource {
  paramNames: string[];
  available: boolean;
  test: (hypothesis: string) => Promise<HypothesisTestResponse | null>;
}

export function regressionHypothesisSource(
  data: RegressionResultData<{ df_residual: number }>,
): HypothesisTestSource {
  const paramNames = buildParamNames(data.coefficients);
  return {
    paramNames,
    available:
      data.betas != null && data.cov_beta != null && data.model_basic_info.df_residual != null,
    test: async (hypothesis) => {
      if (!data.betas || !data.cov_beta) return null;
      return hypothesisTest({
        betas: data.betas,
        cov_beta: data.cov_beta,
        df_residual: data.model_basic_info.df_residual,
        param_names: paramNames,
        hypothesis,
      });
    },
  };
}

export function useHypothesisTestBlock(source: HypothesisTestSource) {
  const { t } = useTranslation();
  const [hypothesis, setHypothesis] = useState("");
  const [result, setResult] = useState<HypothesisTestResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const paramNames = source.paramNames;
  const canRun = source.available && hypothesis.trim().length > 0;

  const run = async () => {
    if (!canRun) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await source.test(hypothesis.trim());
      setResult(res);
    } catch (e) {
      setError(formatInlineUserError(e, t));
    } finally {
      setLoading(false);
    }
  };

  return {
    hypothesis,
    setHypothesis,
    result,
    error,
    loading,
    paramNames,
    canRun,
    run,
  };
}

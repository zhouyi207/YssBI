import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { hypothesisTest } from '@/features/application/stats/statsActions';
import { formatInlineUserError } from '@/features/application/userErrorSummary';
import type { HypothesisTestResponse } from '@/features/application/stats/statsActions';
import { buildParamNames } from '@/shared/stats/regressionReportUtils';
import type { RegressionResultData } from '@/shared/types/report';

export function useHypothesisTestBlock(data: RegressionResultData) {
  const { t } = useTranslation();
  const [hypothesis, setHypothesis] = useState('');
  const [result, setResult] = useState<HypothesisTestResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const paramNames = useMemo(() => buildParamNames(data.coefficients), [data.coefficients]);
  const canRun =
    data.betas != null &&
    data.cov_beta != null &&
    data.model_basic_info.df_residual != null &&
    hypothesis.trim().length > 0;

  const run = async () => {
    if (!canRun || !data.betas || !data.cov_beta) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await hypothesisTest({
        betas: data.betas,
        cov_beta: data.cov_beta,
        df_residual: data.model_basic_info.df_residual,
        param_names: paramNames,
        hypothesis: hypothesis.trim(),
      });
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

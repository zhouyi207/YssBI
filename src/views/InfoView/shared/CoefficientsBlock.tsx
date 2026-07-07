import { SectionHeader } from './RegressionShared';
import { CoefficientTable } from './CoefficientTable';
import { CoeffBarChart } from './CoeffBarChart';
import type { Coefficient } from './types';

const COEFF_ICON = (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
  </svg>
);

const BAR_ICON = (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
  </svg>
);

export interface CoefficientsBlockProps {
  coefficients: Coefficient[];
  hasCategorical: boolean;
  /** AR(1) 自相关参数 ρ，Prais 时传入 */
  ar1Rho?: number;
  /** IV/VAR uses z (asymptotic normal), not t */
  useZStat?: boolean;
  /** Logit: show odds ratio exp(β) */
  showOddsRatio?: boolean;
  /** Override "Category" column label (e.g. "Equation" for VAR) */
  categoryLabel?: string;
}

export function CoefficientsBlock({
  coefficients,
  hasCategorical,
  ar1Rho,
  useZStat,
  showOddsRatio,
  categoryLabel,
}: CoefficientsBlockProps) {
  const significantCount = coefficients.filter((c) => c.is_significant).length;

  return (
    <>
      <SectionHeader
        title={`Coefficients (${significantCount}/${coefficients.length} significant)`}
        icon={COEFF_ICON}
      />
      <CoefficientTable
        coefficients={coefficients}
        hasCategorical={hasCategorical}
        ar1Rho={ar1Rho}
        useZStat={useZStat}
        showOddsRatio={showOddsRatio}
        categoryLabel={categoryLabel}
      />

      <SectionHeader title="Coefficient Magnitude" icon={BAR_ICON} />
      <CoeffBarChart coefficients={coefficients} />
    </>
  );
}

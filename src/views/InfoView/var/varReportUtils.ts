import type { Coefficient, VARSummaryResultData, VARStableRow } from '../shared/types';

export function varCoeffsToOLSFormat(coefficients: VARSummaryResultData['coefficients']): Coefficient[] {
  const eqOrder = [...new Set(coefficients.map((x) => x.eq_name))];
  const mapped = coefficients.map((c, idx) => ({
    variable: c.variable,
    category: c.eq_name,
    coef: c.coef,
    std_err: c.std_err,
    t_value: c.z_value,
    p_value: c.p_value,
    'confidence_interval_0.025': c.ci_lower,
    'confidence_interval_0.975': c.ci_upper,
    is_significant: c.p_value < 0.05,
    _sortKey: c.variable === 'const' ? 0 : 1,
    _eqOrder: eqOrder.indexOf(c.eq_name),
    _idx: idx,
  }));
  mapped.sort((a, b) => {
    if (a._eqOrder !== b._eqOrder) return a._eqOrder - b._eqOrder;
    if (a._sortKey !== b._sortKey) return a._sortKey - b._sortKey;
    return a._idx - b._idx;
  });
  return mapped.map(({ _sortKey, _eqOrder, _idx, ...rest }) => rest as Coefficient);
}

export function sortVarStableRows(rows: VARStableRow[]): VARStableRow[] {
  return [...rows].sort((a, b) => b.modulus - a.modulus);
}

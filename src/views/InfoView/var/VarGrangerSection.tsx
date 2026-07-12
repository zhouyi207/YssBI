import { ReportSection } from '../shared/ReportLayout';
import { formatNum } from '../shared/RegressionShared';
import { VarModelCell, VarModelRow, VarModelTable } from '../shared/VarModelTable';
import type { VARGrangerDisplay } from '@/shared/types/report';

export function VarGrangerSection({ rows }: { rows: VARGrangerDisplay[] }) {
  if (rows.length === 0) return null;

  return (
    <ReportSection title="Granger causality Wald tests" icon="granger">
      <VarModelTable className="mb-6" columns={['Equation', 'Excluded', 'chi2', 'df', 'Prob > chi2']}>
        {rows.map((row, i) => (
          <VarModelRow key={i} className={row.excluded === 'ALL' ? 'border-b-2 border-border' : undefined}>
            <VarModelCell>{row.eq_name}</VarModelCell>
            <VarModelCell>{row.excluded}</VarModelCell>
            <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
            <VarModelCell>{row.df}</VarModelCell>
            <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
          </VarModelRow>
        ))}
      </VarModelTable>
    </ReportSection>
  );
}

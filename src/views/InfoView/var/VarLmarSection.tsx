import { ReportSection } from '../shared/ReportLayout';
import { formatNum } from '../shared/RegressionShared';
import { VarModelCell, VarModelRow, VarModelTable } from '../shared/VarModelTable';
import type { VARLmarDisplay } from '../shared/types';

export function VarLmarSection({ rows }: { rows: VARLmarDisplay[] }) {
  if (rows.length === 0) return null;

  return (
    <ReportSection title="Lagrange-multiplier test (varlmar)" icon="margins">
      <VarModelTable
        className="mb-6"
        columns={['lag', 'chi2', 'df', 'Prob > chi2']}
        footer={
          <div className="border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
            H0: no autocorrelation at lag order
          </div>
        }
      >
        {rows.map((row, i) => (
          <VarModelRow key={i}>
            <VarModelCell>{row.lag}</VarModelCell>
            <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
            <VarModelCell>{row.df}</VarModelCell>
            <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
          </VarModelRow>
        ))}
      </VarModelTable>
    </ReportSection>
  );
}

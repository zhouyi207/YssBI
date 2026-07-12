import { useMemo } from 'react';
import { ReportSection } from '../shared/ReportLayout';
import { formatNum } from '../shared/RegressionShared';
import { VarModelCell, VarModelRow, VarModelTable } from '../shared/VarModelTable';
import type { VARWleDisplay } from '@/shared/types/report';

export function VarWaldLagExclusionSection({ rows }: { rows: VARWleDisplay[] }) {
  const byEquation = useMemo(() => {
    const grouped = rows.reduce<Record<string, VARWleDisplay[]>>((acc, row) => {
      (acc[row.eq_name] ??= []).push(row);
      return acc;
    }, {});
    return [...new Set(rows.map((r) => r.eq_name))].map((eqName) => ({
      eqName,
      rows: grouped[eqName] ?? [],
    }));
  }, [rows]);

  if (rows.length === 0) return null;

  return (
    <ReportSection title="Wald lag-exclusion statistics (varwle)" icon="wald">
      <div className="mb-6 space-y-4">
        {byEquation.map(({ eqName, rows: eqRows }) => (
          <div key={eqName} className="overflow-hidden rounded-lg border border-border bg-muted">
            <div className="border-b border-border px-4 py-2.5 text-sm font-medium text-foreground">
              Equation: {eqName}
            </div>
            <VarModelTable columns={['lag', 'chi2', 'df', 'Prob > chi2']}>
              {eqRows.map((row, i) => (
                <VarModelRow key={i}>
                  <VarModelCell>{row.lag}</VarModelCell>
                  <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
                  <VarModelCell>{row.df}</VarModelCell>
                  <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
                </VarModelRow>
              ))}
            </VarModelTable>
          </div>
        ))}
      </div>
    </ReportSection>
  );
}

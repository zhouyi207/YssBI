import { ReportSection } from '../shared/ReportLayout';
import { formatNum } from '../shared/RegressionShared';
import { VarModelCell, VarModelRow, VarModelTable } from '../shared/VarModelTable';
import type { VAREquationDisplay } from '../shared/types';

export function VarEquationSummarySection({ equations }: { equations: VAREquationDisplay[] }) {
  if (equations.length === 0) return null;

  return (
    <ReportSection title="Equation Summary" icon="anova">
      <VarModelTable className="mb-6" columns={['Equation', 'Parms', 'RMSE', 'R-sq', 'chi2', 'P>chi2']}>
        {equations.map((eq, i) => (
          <VarModelRow key={i}>
            <VarModelCell>{eq.eq_name}</VarModelCell>
            <VarModelCell>{eq.parms}</VarModelCell>
            <VarModelCell>{formatNum(eq.rmse)}</VarModelCell>
            <VarModelCell>{formatNum(eq.r_sq)}</VarModelCell>
            <VarModelCell>{formatNum(eq.chi2)}</VarModelCell>
            <VarModelCell>{formatNum(eq.p_chi2)}</VarModelCell>
          </VarModelRow>
        ))}
      </VarModelTable>
    </ReportSection>
  );
}

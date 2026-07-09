import { ReportSection } from '../shared/ReportLayout';
import { VarEigenvalueStabilityPanel } from '../shared/VarEigenvalueStabilityPanel';
import type { VARStableRow } from '@/shared/types/report';

export function VarStabilitySection({ rows }: { rows: VARStableRow[] }) {
  if (rows.length === 0) return null;

  return (
    <ReportSection title="Eigenvalue stability condition" icon="margins">
      <VarEigenvalueStabilityPanel
        rows={rows}
        unstableMessage="At least one eigenvalue is at least 1.0. VAR does not satisfy stability condition."
      />
    </ReportSection>
  );
}

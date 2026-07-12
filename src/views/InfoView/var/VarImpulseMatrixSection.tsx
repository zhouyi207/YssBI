import { TableBody, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { ReportSection } from '../shared/ReportLayout';
import type { ReportSectionIcon } from '../shared/reportIcons';
import { formatNum } from '../shared/RegressionShared';
import { InfoStatsTable } from '../shared/InfoStatsTable';
import { infoVarHeadClass, VarModelCell, VarModelRow } from '../shared/VarModelTable';

export function VarImpulseMatrixSection({
  title,
  icon,
  stepHeader,
  varNames,
  steps,
}: {
  title: string;
  icon: ReportSectionIcon;
  stepHeader: string;
  varNames: string[];
  steps: number[][][];
}) {
  if (steps.length === 0 || varNames.length === 0) return null;

  return (
    <ReportSection title={title} icon={icon}>
      <InfoStatsTable className="mb-6 overflow-x-auto bg-muted" tableClassName="min-w-[400px] text-left text-sm">
        <TableHeader>
          <TableRow className="border-b border-border hover:bg-transparent">
            <TableHead className={infoVarHeadClass}>{stepHeader}</TableHead>
            {varNames.flatMap((imp) =>
              varNames.map((resp) => (
                <TableHead key={`${imp}-${resp}`} className={infoVarHeadClass}>
                  {imp}→{resp}
                </TableHead>
              )),
            )}
          </TableRow>
        </TableHeader>
        <TableBody>
          {steps.map((stepData, stepIndex) => (
            <VarModelRow key={stepIndex}>
              <VarModelCell>{stepIndex}</VarModelCell>
              {varNames.flatMap((_, impIdx) =>
                varNames.map((_, respIdx) => (
                  <VarModelCell key={`${impIdx}-${respIdx}`}>
                    {formatNum(stepData[respIdx]?.[impIdx] ?? 0)}
                  </VarModelCell>
                )),
              )}
            </VarModelRow>
          ))}
        </TableBody>
      </InfoStatsTable>
    </ReportSection>
  );
}

import { toVarStabilityPoints } from "@/features/application/stats/toVarStabilityPoints";
import {
  VarStabilityChart,
  type VarStabilityPoint,
  type VarStabilityValueField,
} from "@/shared/charts/statistical";
import type { VARStableRow } from "@/shared/types/report";
import { formatNum } from "./utils";
import { VarEigenvalueTable } from "./VarModelTable";

function formatVarStabilityValue(value: number, field: VarStabilityValueField): string {
  return field === "modulus" ? formatNum(value, 6) : formatNum(value);
}

function getVarPointLabel(index: number): string {
  return `Eigenvalue ${index + 1}`;
}

function getVarPointAriaLabel(point: VarStabilityPoint, index: number): string {
  return `${getVarPointLabel(index)}, real ${formatNum(point.re)}, imaginary ${formatNum(point.im)}, modulus ${formatNum(point.modulus, 6)}, ${point.status}`;
}

export function VarEigenvalueStabilityPanel({
  rows,
  unstableMessage,
  stableMessage = "All the eigenvalues lie inside the unit circle.",
}: {
  rows: VARStableRow[];
  unstableMessage: string;
  stableMessage?: string;
}) {
  const points = toVarStabilityPoints(rows);
  const isUnstable = points.some((point) => point.status === "unstable");

  return (
    <div className="mb-6 grid min-h-[360px] grid-cols-[auto_1fr] items-stretch gap-4">
      <div className="flex h-full flex-col overflow-hidden rounded-lg border border-border bg-muted">
        <div className="flex min-h-0 flex-1 flex-col">
          <VarEigenvalueTable rows={rows} />
          <div className="min-h-0 flex-1 bg-muted" />
        </div>
        <div className="shrink-0 border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
          {isUnstable ? unstableMessage : stableMessage}
        </div>
      </div>
      <div className="flex min-h-0 min-w-[240px]">
        <VarStabilityChart
          data={points}
          xLabel="Real"
          yLabel="Imaginary"
          ariaLabel="Eigenvalue stability chart"
          getPointLabel={getVarPointLabel}
          getPointAriaLabel={getVarPointAriaLabel}
          modulusLabel="Modulus"
          unstableTooltipLabel="≥ 1 (unstable)"
          formatValue={formatVarStabilityValue}
        />
      </div>
    </div>
  );
}

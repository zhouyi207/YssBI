import { VARStableChart, VarEigenvalueTable } from './VarModelTable';
import type { VARStableRow } from './types';

export function VarEigenvalueStabilityPanel({
  rows,
  unstableMessage,
  stableMessage = 'All the eigenvalues lie inside the unit circle.',
}: {
  rows: VARStableRow[];
  unstableMessage: string;
  stableMessage?: string;
}) {
  const isUnstable = rows.some((r) => r.modulus >= 1);

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
        <VARStableChart data={rows} />
      </div>
    </div>
  );
}

import { useMemo } from "react";
import { DataTable } from "@/components/ui-presentation/DataTable";
import { useStructuredReportTable } from "@/features/application/results/useStructuredReportTable";
import { ResultPageToolbar } from "@/features/application/results/components/ResultPageToolbar";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import type { ResultReference } from "@/shared/types/domain/result";
import type {
  ReportTableSection,
  StabilityRoot,
} from "@/shared/types/domain/structuredReportDisplay";

const EMPTY_TABLE_ROWS: readonly Readonly<Record<string, unknown>>[] = [];

function StabilityCircle({ roots }: { roots: readonly StabilityRoot[] }) {
  const extent = Math.max(
    1.2,
    ...roots.flatMap((row) => [Math.abs(row.re), Math.abs(row.im)]).map((value) => value * 1.15),
  );
  const scale = 170 / extent;
  return (
    <svg
      viewBox="0 0 400 400"
      role="img"
      aria-label="Stability roots and unit circle"
      className="mx-auto max-h-96 w-full max-w-96 text-foreground"
    >
      <circle cx={200} cy={200} r={scale} fill="none" stroke="currentColor" opacity={0.5} />
      <path d="M20 200H380M200 20V380" fill="none" stroke="currentColor" opacity={0.3} />
      <text x={365} y={220} fontSize={12} fill="currentColor">
        Real
      </text>
      <text x={210} y={25} fontSize={12} fill="currentColor">
        Imaginary
      </text>
      <text x={200 + scale} y={216} fontSize={11} fill="currentColor">
        1
      </text>
      {roots.map((row, index) => (
        <circle
          key={index}
          cx={200 + row.re * scale}
          cy={200 - row.im * scale}
          r={4}
          fill="currentColor"
        >
          <title>{`Real: ${row.re}; imaginary: ${row.im}; modulus: ${row.modulus ?? "unavailable"}`}</title>
        </circle>
      ))}
    </svg>
  );
}

export function StructuredReportTable({
  reference,
  section,
}: {
  reference: ResultReference;
  section: ReportTableSection;
}) {
  const { page, content } = useStructuredReportTable(reference, section);
  const values = content?.kind === "table" ? content.rows : EMPTY_TABLE_ROWS;
  const columns = useMemo(
    () =>
      Object.entries(section.columns).map(([id, label]) => ({
        id,
        label,
        format:
          values.length > 0 &&
          values.every((row) => row[id] === null || typeof row[id] === "number")
            ? ("number" as const)
            : ("text" as const),
      })),
    [section.columns, values],
  );
  const rows = useMemo(
    () =>
      values.map((row) =>
        columns.map(({ id }) => {
          const value = row[id];
          return typeof value === "number" || typeof value === "string"
            ? value
            : value === null || value === undefined
              ? "—"
              : String(value);
        }),
      ),
    [values, columns],
  );
  return (
    <div className="min-w-0 space-y-3">
      {page.error && <ResultReadError error={page.error} onRetry={() => void page.reload()} />}
      {page.loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : content === null ? (
        <p role="alert">
          {section.kind === "stability"
            ? "Invalid stability roots"
            : "Report table fields do not match the declared columns."}
        </p>
      ) : content.kind === "stability" ? (
        <StabilityCircle roots={content.rows} />
      ) : (
        <div className="overflow-x-auto">
          <DataTable columns={columns} rows={rows} />
        </div>
      )}
      {section.kind === "stability" && section.value.rowCount > 100 && (
        <p className="text-xs text-muted-foreground">
          The circle shows this page of roots; use pagination to inspect the remaining roots.
        </p>
      )}
      <ResultPageToolbar {...page} onPrevious={page.goToPreviousPage} onNext={page.goToNextPage} />
    </div>
  );
}

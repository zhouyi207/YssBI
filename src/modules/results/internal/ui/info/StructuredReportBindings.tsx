import { useMemo } from "react";
import { DataTable } from "@/components/ui-presentation/DataTable";
import { usePagedResultRows } from "@/features/application/results/usePagedResultRows";
import { ResultPageToolbar } from "@/features/application/results/components/ResultPageToolbar";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import type { ResultReference } from "@/shared/types/domain/result";
import type {
  ResultTableReference,
  StructuredResultPart,
} from "@/shared/types/domain/resultReport";
import type { ReportDisplaySection } from "@/shared/types/domain/structuredReportDisplay";
import { isRecord } from "@/shared/types/report/guards";

function StabilityCircle({ rows }: { rows: readonly unknown[] }) {
  const roots = rows.filter((row): row is Record<string, unknown> => isRecord(row));
  if (
    roots.some(
      (row) =>
        typeof row.re !== "number" ||
        !Number.isFinite(row.re) ||
        typeof row.im !== "number" ||
        !Number.isFinite(row.im),
    )
  )
    return <p role="alert">Invalid stability roots</p>;
  const extent = Math.max(
    1.2,
    ...roots
      .flatMap((row) => [Math.abs(row.re as number), Math.abs(row.im as number)])
      .map((value) => value * 1.15),
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
          cx={200 + (row.re as number) * scale}
          cy={200 - (row.im as number) * scale}
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
  table,
  section,
}: {
  reference: ResultReference;
  table: ResultTableReference<StructuredResultPart>;
  section: ReportDisplaySection;
}) {
  const page = usePagedResultRows(reference, table.rowCount, 100, undefined, table.part);
  const values = useMemo(() => page.rows.map((row) => row[0]), [page.rows]);
  const columns = useMemo(
    () =>
      Object.entries(section.columns).map(([id, label]) => ({
        id,
        label,
        format:
          values.length > 0 &&
          values.every((row) => isRecord(row) && (row[id] === null || typeof row[id] === "number"))
            ? ("number" as const)
            : ("text" as const),
      })),
    [section.columns, values],
  );
  const rows = useMemo(
    () =>
      values.map((row) =>
        columns.map(({ id }) => {
          const value = isRecord(row) ? row[id] : undefined;
          return typeof value === "number" || typeof value === "string"
            ? value
            : value === null || value === undefined
              ? "—"
              : String(value);
        }),
      ),
    [values, columns],
  );
  if (
    section.kind === "table" &&
    values.some(
      (row) =>
        !isRecord(row) ||
        columns.some(
          ({ id }) =>
            !Object.prototype.hasOwnProperty.call(row, id) ||
            (row[id] !== null && typeof row[id] === "object"),
        ),
    )
  )
    return <p role="alert">Report table fields do not match the declared columns.</p>;
  return (
    <div className="min-w-0 space-y-3">
      {page.error && <ResultReadError error={page.error} onRetry={() => void page.reload()} />}
      {page.loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : section.kind === "stability" ? (
        <StabilityCircle rows={values} />
      ) : (
        <div className="overflow-x-auto">
          <DataTable columns={columns} rows={rows} />
        </div>
      )}
      {section.kind === "stability" && table.rowCount > 100 && (
        <p className="text-xs text-muted-foreground">
          The circle shows this page of roots; use pagination to inspect the remaining roots.
        </p>
      )}
      <ResultPageToolbar {...page} onPrevious={page.goToPreviousPage} onNext={page.goToNextPage} />
    </div>
  );
}

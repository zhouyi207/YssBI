import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { isRecord } from "@/shared/types/report/guards";
import type { UiMetric, UiTableData, UiValue } from "@/shared/types/domain/uiData";
import { DataTable } from "./DataTable";
import { KeyValue } from "./KeyValue";
import { Section } from "./Section";

const PAGE_SIZE = 100;
const scalar = (value: unknown): boolean => value === null || typeof value !== "object";
const cell = (value: unknown): UiValue =>
  typeof value === "number" || typeof value === "string" ? value : String(value);

function metric(label: string, value: unknown): UiMetric {
  return {
    id: label,
    label,
    value: cell(value),
    format: typeof value === "number" ? "number" : "text",
  };
}

function tableFor(values: readonly unknown[]): Omit<UiTableData, "kind"> | null {
  if (values.every(scalar)) {
    return {
      columns: [{ id: "value", label: "value", format: "text" }],
      rows: values.map((value) => [cell(value)]),
    };
  }
  if (values.every((value) => Array.isArray(value) && value.every(scalar))) {
    const rows = values as unknown[][];
    const width = rows.reduce((maximum, row) => Math.max(maximum, row.length), 0);
    return {
      columns: Array.from({ length: width }, (_, index) => ({
        id: String(index),
        label: String(index + 1),
        format: "text" as const,
      })),
      rows: rows.map((row) =>
        Array.from({ length: width }, (_, index) => (index < row.length ? cell(row[index]) : "")),
      ),
    };
  }
  if (values.every((value) => isRecord(value) && Object.values(value).every(scalar))) {
    const records = values as Record<string, unknown>[];
    const keys = [...new Set(records.flatMap(Object.keys))];
    return {
      columns: keys.map((key) => ({ id: key, label: key, format: "text" as const })),
      rows: records.map((record) =>
        keys.map((key) =>
          Object.prototype.hasOwnProperty.call(record, key) ? cell(record[key]) : "",
        ),
      ),
    };
  }
  return null;
}

function StructuredList({ values, depth }: { values: readonly unknown[]; depth: number }) {
  const { t } = useTranslation();
  const [page, setPage] = useState(0);
  const offset = Math.min(page, Math.max(0, Math.ceil(values.length / PAGE_SIZE) - 1)) * PAGE_SIZE;
  const visible = useMemo(() => values.slice(offset, offset + PAGE_SIZE), [values, offset]);
  const table = useMemo(() => tableFor(visible), [visible]);
  return (
    <div className="min-w-0 space-y-3">
      {table ? (
        <DataTable {...table} />
      ) : (
        visible.map((value, index) => (
          <Section key={offset + index} title={String(offset + index + 1)} collapsible>
            <StructuredData value={value} depth={depth + 1} />
          </Section>
        ))
      )}
      {values.length > PAGE_SIZE && (
        <div className="flex items-center justify-end gap-2 text-xs text-muted-foreground">
          <span>
            {offset + 1}–{Math.min(offset + PAGE_SIZE, values.length)} / {values.length}
          </span>
          <Button
            size="xs"
            variant="outline"
            disabled={offset === 0}
            onClick={() => setPage((value) => value - 1)}
          >
            {t("sourceInspector.previous")}
          </Button>
          <Button
            size="xs"
            variant="outline"
            disabled={offset + PAGE_SIZE >= values.length}
            onClick={() => setPage((value) => value + 1)}
          >
            {t("sourceInspector.next")}
          </Button>
        </div>
      )}
    </div>
  );
}

/** Present the result's JSON shape without a statistical-method-specific view. */
export function StructuredData({ value, depth = 0 }: { value: unknown; depth?: number }) {
  if (Array.isArray(value)) {
    return value.length ? <StructuredList values={value} depth={depth} /> : <code>[]</code>;
  }
  if (isRecord(value)) {
    const entries = Object.entries(value);
    const metrics = entries
      .filter(([, item]) => scalar(item))
      .map(([key, item]) => metric(key, item));
    return (
      <div className="min-w-0 space-y-4">
        {metrics.length > 0 && <KeyValue items={metrics} />}
        {entries
          .filter(([, item]) => !scalar(item))
          .map(([key, item]) => (
            <Section key={key} title={key} collapsible={depth > 0}>
              <StructuredData value={item} depth={depth + 1} />
            </Section>
          ))}
        {entries.length === 0 && <code>{"{}"}</code>}
      </div>
    );
  }
  return <KeyValue items={[metric("value", value)]} />;
}

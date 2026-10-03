import {
  isStructuredTableReference,
  type ResultTableReference,
  type StructuredResultPart,
} from "./resultReport";
import { isRecord } from "@/shared/types/report/guards";

export type ReportDisplaySection = {
  readonly title: string;
  readonly path: string;
  readonly columns: Readonly<Record<string, string>>;
} & (
  | { readonly kind: "equation"; readonly value: string }
  | {
      readonly kind: "table" | "stability";
      readonly value: ResultTableReference<StructuredResultPart>;
    }
);

export type ReportTableSection = Exclude<ReportDisplaySection, { kind: "equation" }>;
export type StabilityRoot = Readonly<Record<string, unknown>> & {
  readonly re: number;
  readonly im: number;
};
export type StructuredReportRows =
  | { readonly kind: "table"; readonly rows: readonly Readonly<Record<string, unknown>>[] }
  | { readonly kind: "stability"; readonly rows: readonly StabilityRoot[] };

function isStabilityRoot(row: unknown): row is StabilityRoot {
  return (
    isRecord(row) &&
    typeof row.re === "number" &&
    Number.isFinite(row.re) &&
    typeof row.im === "number" &&
    Number.isFinite(row.im)
  );
}

export function parseStructuredReportRows(
  section: ReportTableSection,
  rows: readonly (readonly unknown[])[],
): StructuredReportRows | null {
  const values = rows.map((row) => row[0]);
  if (section.kind === "stability") {
    return values.every(isStabilityRoot) ? { kind: "stability", rows: values } : null;
  }
  const fields = Object.keys(section.columns);
  if (
    !values.every(isRecord) ||
    values.some((row) =>
      fields.some(
        (field) =>
          !Object.prototype.hasOwnProperty.call(row, field) ||
          (row[field] !== null && typeof row[field] === "object"),
      ),
    )
  )
    return null;
  return { kind: "table", rows: values };
}

export function structuredValueAt(root: unknown, path: string): unknown {
  if (!path.startsWith("/") || path.length > 4096 || /~(?:[^01]|$)/.test(path)) return undefined;
  const tokens = path.slice(1).split("/");
  if (tokens.length > 64) return undefined;
  let value: unknown = root;
  for (const part of tokens) {
    const key = part.replace(/~1/g, "/").replace(/~0/g, "~");
    if (!isRecord(value) || !Object.prototype.hasOwnProperty.call(value, key)) return undefined;
    value = value[key];
  }
  return value;
}

/** Parse bounded, server-authored layout metadata; values remain in Results. */
export function parseReportDisplay(root: unknown): Readonly<Record<string, ReportDisplaySection>> {
  if (!isRecord(root) || !("report_display" in root)) return {};
  const display = root.report_display;
  if (
    !isRecord(display) ||
    Object.keys(display).length !== 1 ||
    !isRecord(display.sections) ||
    Object.keys(display.sections).length > 24
  )
    throw new Error("invalid_report_display");
  return Object.fromEntries(
    Object.entries(display.sections).map(([id, item]): [string, ReportDisplaySection] => {
      if (
        !/^[a-zA-Z0-9_-]{1,48}$/.test(id) ||
        !isRecord(item) ||
        Object.keys(item).some((key) => !["title", "kind", "path", "columns"].includes(key)) ||
        typeof item.title !== "string" ||
        new TextEncoder().encode(item.title).length > 200 ||
        (item.kind !== "table" && item.kind !== "equation" && item.kind !== "stability") ||
        typeof item.path !== "string"
      )
        throw new Error("invalid_report_display");
      const columns = item.columns ?? {};
      if (
        !isRecord(columns) ||
        Object.keys(columns).length > 32 ||
        Object.entries(columns).some(
          ([key, label]) =>
            new TextEncoder().encode(key).length > 100 ||
            typeof label !== "string" ||
            new TextEncoder().encode(label).length > 100,
        )
      )
        throw new Error("invalid_report_display");
      if (item.kind === "table" && Object.keys(columns).length === 0)
        throw new Error("invalid_report_display");
      const section = {
        title: item.title,
        path: item.path,
        columns: columns as Record<string, string>,
      };
      const target = structuredValueAt(root, item.path);
      if (item.kind === "equation") {
        if (typeof target !== "string" || new TextEncoder().encode(target).length > 16384)
          throw new Error("invalid_report_display");
        return [id, { ...section, kind: item.kind, value: target }];
      }
      if (!isStructuredTableReference(target) || target.part !== `structured:${item.path}`) {
        throw new Error("invalid_report_display");
      }
      return [id, { ...section, kind: item.kind, value: target }];
    }),
  );
}

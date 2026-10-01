import { isStructuredTableReference } from "./resultReport";
import { isRecord } from "@/shared/types/report/guards";

export interface ReportDisplaySection {
  readonly title: string;
  readonly kind: "table" | "equation" | "stability";
  readonly path: string;
  readonly columns: Readonly<Record<string, string>>;
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
    Object.entries(display.sections).map(([id, item]) => {
      if (
        !/^[a-zA-Z0-9_-]{1,48}$/.test(id) ||
        !isRecord(item) ||
        Object.keys(item).some((key) => !["title", "kind", "path", "columns"].includes(key)) ||
        typeof item.title !== "string" ||
        new TextEncoder().encode(item.title).length > 200 ||
        !["table", "equation", "stability"].includes(String(item.kind)) ||
        typeof item.path !== "string" ||
        structuredValueAt(root, item.path) === undefined
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
      const target = structuredValueAt(root, item.path);
      if (item.kind === "equation") {
        if (typeof target !== "string" || new TextEncoder().encode(target).length > 16384)
          throw new Error("invalid_report_display");
      } else if (!isStructuredTableReference(target) || target.part !== `structured:${item.path}`) {
        throw new Error("invalid_report_display");
      }
      return [
        id,
        {
          title: item.title,
          kind: item.kind as ReportDisplaySection["kind"],
          path: item.path,
          columns: columns as Record<string, string>,
        },
      ];
    }),
  );
}

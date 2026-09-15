import { isRecord } from "@/shared/types/report/guards";
import { isResultReference, resultReference, type ResultReference } from "./result";

export const OLS_REPORT_SECTION_KINDS = [
  "equation",
  "modelSummary",
  "anova",
  "coefficientTable",
  "hypothesisTest",
  "diagnostics",
  "residualPlot",
  "observations",
  "acfPacf",
  "serialTests",
] as const;

export type OlsReportSectionKind = (typeof OLS_REPORT_SECTION_KINDS)[number];
export interface OlsReportSection {
  readonly id: string;
  readonly kind: OlsReportSectionKind;
  readonly visible: boolean;
}

/** Presentation only. Numerical values and query parameters belong to Results and its controls. */
export interface OlsReportSpec {
  readonly schemaVersion: 1;
  readonly type: "regressionReport";
  readonly source: ResultReference;
  readonly sections: readonly OlsReportSection[];
}

export const OLS_REPORT_SPEC_TEXT_LIMIT = 16 * 1024;
export type OlsReportSpecErrorCode =
  | "invalidJson"
  | "tooLarge"
  | "invalidShape"
  | "unsupportedVersion"
  | "sourceMismatch"
  | "unknownSection"
  | "invalidId"
  | "duplicateSection";
export interface OlsReportSpecIssue {
  readonly code: OlsReportSpecErrorCode;
  readonly path: string;
}
export type OlsReportSpecResult =
  | { readonly ok: true; readonly value: OlsReportSpec }
  | { readonly ok: false; readonly issue: OlsReportSpecIssue };

export function defaultOlsReportSpec(source: ResultReference): OlsReportSpec {
  return {
    schemaVersion: 1,
    type: "regressionReport",
    source: resultReference(source),
    sections: OLS_REPORT_SECTION_KINDS.map((kind) => ({ id: kind, kind, visible: true })),
  };
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key));
}

export function parseOlsReportSpec(raw: unknown, source: ResultReference): OlsReportSpecResult {
  const fail = (code: OlsReportSpecErrorCode, path: string): OlsReportSpecResult => ({
    ok: false,
    issue: { code, path },
  });
  if (!isRecord(raw) || !hasOnlyKeys(raw, ["schemaVersion", "type", "source", "sections"]))
    return fail("invalidShape", "$");
  if (raw.schemaVersion !== 1) return fail("unsupportedVersion", "schemaVersion");
  if (raw.type !== "regressionReport") return fail("invalidShape", "type");
  if (
    !isRecord(raw.source) ||
    !hasOnlyKeys(raw.source, ["executionSessionId", "resultId"]) ||
    !isResultReference(raw.source) ||
    raw.source.executionSessionId !== source.executionSessionId ||
    raw.source.resultId !== source.resultId
  )
    return fail("sourceMismatch", "source");
  if (!Array.isArray(raw.sections)) return fail("invalidShape", "sections");
  if (raw.sections.length > OLS_REPORT_SECTION_KINDS.length) return fail("tooLarge", "sections");

  const sections: OlsReportSection[] = [];
  const ids = new Set<string>();
  const kinds = new Set<string>();
  for (const [index, section] of raw.sections.entries()) {
    const path = `sections[${index}]`;
    // A flat, closed vocabulary also excludes nested trees, cycles, actions and arbitrary props.
    if (!isRecord(section) || !hasOnlyKeys(section, ["id", "kind", "visible"]))
      return fail("invalidShape", path);
    if (typeof section.id !== "string" || !/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(section.id))
      return fail("invalidId", `${path}.id`);
    const kind = OLS_REPORT_SECTION_KINDS.find((kind) => kind === section.kind);
    if (!kind) return fail("unknownSection", `${path}.kind`);
    if (section.visible !== undefined && typeof section.visible !== "boolean")
      return fail("invalidShape", `${path}.visible`);
    if (ids.has(section.id) || kinds.has(kind)) return fail("duplicateSection", path);
    ids.add(section.id);
    kinds.add(kind);
    sections.push({ id: section.id, kind, visible: section.visible ?? true });
  }
  return {
    ok: true,
    value: {
      schemaVersion: 1,
      type: "regressionReport",
      source: resultReference(source),
      sections,
    },
  };
}

export function parseOlsReportSpecJson(text: string, source: ResultReference): OlsReportSpecResult {
  if (text.length > OLS_REPORT_SPEC_TEXT_LIMIT)
    return { ok: false, issue: { code: "tooLarge", path: "$" } };
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, issue: { code: "invalidJson", path: "$" } };
  }
  return parseOlsReportSpec(raw, source);
}

import { isFiniteNumber, isNonNegativeInteger, isRecord, isString } from "./guards";

export interface ReportFieldIssue {
  readonly fieldPath: string;
  readonly reason: string;
}

export type ReportFieldResult<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly issue: ReportFieldIssue };

export interface ReportField<T> {
  read(value: unknown, path: string): ReportFieldResult<T>;
}

function invalid(value: unknown, path: string, expected: string): ReportFieldResult<never> {
  return {
    ok: false,
    issue: {
      fieldPath: path,
      reason: value === undefined ? "missing required field" : `expected ${expected}`,
    },
  };
}

function scalarField<T>(expected: string, guard: (value: unknown) => value is T): ReportField<T> {
  return {
    read: (value, path) => (guard(value) ? { ok: true, value } : invalid(value, path, expected)),
  };
}

export const numberField = scalarField("finite number", isFiniteNumber);
export const integerField = scalarField("non-negative integer", isNonNegativeInteger);
export const stringField = scalarField("string", isString);
export const booleanField = scalarField(
  "boolean",
  (value): value is boolean => typeof value === "boolean",
);

export function literalField<const T extends string>(...values: readonly T[]): ReportField<T> {
  return scalarField(
    values.map((value) => JSON.stringify(value)).join(" or "),
    (value): value is T =>
      typeof value === "string" && (values as readonly string[]).includes(value),
  );
}

export function optionalField<T>(field: ReportField<T>): ReportField<T | undefined> {
  return {
    read: (value, path) => (value === undefined ? { ok: true, value } : field.read(value, path)),
  };
}

export function nullableField<T>(field: ReportField<T>): ReportField<T | null> {
  return {
    read: (value, path) => (value === null ? { ok: true, value } : field.read(value, path)),
  };
}

export function arrayField<T>(field: ReportField<T>): ReportField<T[]> {
  return {
    read(value, path) {
      if (!Array.isArray(value)) return invalid(value, path, "array");
      const result: T[] = [];
      for (let index = 0; index < value.length; index++) {
        const item = field.read(value[index], `${path}[${index}]`);
        if (!item.ok) return item;
        result.push(item.value);
      }
      return { ok: true, value: result };
    },
  };
}

export function objectField<T extends object>(fields: {
  readonly [K in keyof T]-?: ReportField<T[K]>;
}): ReportField<T> {
  return {
    read(value, path) {
      if (!isRecord(value)) return invalid(value, path, "object");
      const result: Partial<T> = {};
      for (const key of Object.keys(fields) as (keyof T & string)[]) {
        const parsed = fields[key].read(value[key], path === "$" ? key : `${path}.${key}`);
        if (!parsed.ok) return parsed;
        if (parsed.value !== undefined) result[key] = parsed.value;
      }
      return { ok: true, value: result as T };
    },
  };
}

export function refineField<T>(
  field: ReportField<T>,
  validate: (value: T) => ReportFieldIssue | null,
): ReportField<T> {
  return {
    read(value, path) {
      const parsed = field.read(value, path);
      if (!parsed.ok) return parsed;
      const issue = validate(parsed.value);
      if (!issue) return parsed;
      return {
        ok: false,
        issue: {
          ...issue,
          fieldPath: path === "$" ? issue.fieldPath : `${path}.${issue.fieldPath}`,
        },
      };
    },
  };
}

export function parsedField<T>(
  parse: (value: unknown) => T | null,
  expected: string,
): ReportField<T> {
  return {
    read(value, path) {
      const parsed = parse(value);
      return parsed === null ? invalid(value, path, expected) : { ok: true, value: parsed };
    },
  };
}

export function readReportField<T>(field: ReportField<T>, value: unknown): T | null {
  const parsed = field.read(value, "$");
  return parsed.ok ? parsed.value : null;
}

export const SEMANTIC_TYPES = [
  "Numeric",
  "Categorical",
  "Ordinal",
  "Binary",
  "Datetime",
  "Text",
  "Identifier",
] as const;

export type SemanticType = (typeof SEMANTIC_TYPES)[number];

export interface SemanticValue {
  value: string;
  label: string;
}

export interface ColumnSemantic {
  kind: SemanticType;
  values: readonly SemanticValue[];
  positiveValue: string | null;
  numeric: { integer: boolean; minimum: string | null; maximum: string | null } | null;
}

export function isColumnSemantic(value: unknown): value is ColumnSemantic {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  const numeric = candidate.numeric;
  return (
    SEMANTIC_TYPES.includes(candidate.kind as SemanticType) &&
    Array.isArray(candidate.values) &&
    candidate.values.every(
      (entry) =>
        entry &&
        typeof entry === "object" &&
        typeof entry.value === "string" &&
        typeof entry.label === "string",
    ) &&
    (candidate.positiveValue === null || typeof candidate.positiveValue === "string") &&
    (numeric === null ||
      (typeof numeric === "object" &&
        numeric !== null &&
        "integer" in numeric &&
        typeof numeric.integer === "boolean" &&
        "minimum" in numeric &&
        (numeric.minimum === null || typeof numeric.minimum === "string") &&
        "maximum" in numeric &&
        (numeric.maximum === null || typeof numeric.maximum === "string")))
  );
}

export interface ColumnInfo {
  name: string;
  type: string;
  physical?: string;
  semantic?: ColumnSemantic | null;
}

/** Read-only projection of an installed sample; resource paths stay in Rust. */
export interface SampleDatasetSummary {
  id: string;
  name: string;
  version: number;
  rowCount: number;
  columnCount: number;
  byteSize: number;
}

export type DatabaseCellValue = string | number | boolean | null;
export type DatabaseRow = DatabaseCellValue[];

export interface LoadDatabaseResult {
  id: string;
  name: string;
  rowCount: number;
  columnCount: number;
  columns: ColumnInfo[];
}

export type DatabaseImportSqlEngineDTO = "sqlite" | "postgres" | "mysql";

export type CsvEngineConfig = {
  path: string;
  delimiter?: string;
  hasHeader?: boolean;
  inferSchemaLength?: number;
};

export type ParquetEngineConfig = { path: string; columns?: string[] };
export type ExcelEngineConfig = { path: string; sheet: string };
export type DatabaseEngineDTO = { dataset: Record<string, never> };

export type DatabaseImportSourceDTO =
  | { sql: { engine: DatabaseImportSqlEngineDTO; connectionString: string; table: string } }
  | { csv: CsvEngineConfig }
  | { parquet: ParquetEngineConfig }
  | { excel: ExcelEngineConfig };

export interface DatabaseDeclDTO {
  id: string;
  resourcePath?: string;
  name?: string;
  engine?: DatabaseEngineDTO;
  schemaVersion?: number;
  required?: boolean;
  columns?: ColumnInfo[];
  rowCount?: number;
  columnCount?: number;
  loadFailed?: boolean;
}

export type DatabaseDecl = DatabaseDeclDTO;

export interface DatabaseDocumentDto {
  id: string;
  engine: DatabaseEngineDTO;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}

/** Frontend database projection with a display name resolved by Application. */
export type DatabaseRecord = DatabaseDeclDTO & { name: string };

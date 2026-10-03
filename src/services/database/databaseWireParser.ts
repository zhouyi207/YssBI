import { z } from "zod";
import {
  isColumnSemantic,
  isDatabaseEngine,
  type ColumnSemantic,
  type DatabaseEngineDTO,
} from "@/shared/types/domain/database";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";

const count = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const columns = z
  .array(
    z.strictObject({
      name: z.string(),
      type: z.string(),
      physical: z.string(),
      semantic: z.custom<ColumnSemantic>(isColumnSemantic).nullable(),
    }),
  )
  .refine((value) => new Set(value.map((column) => column.name)).size === value.length);

export const databaseMetadataSchema = z
  .strictObject({
    id: z.string().min(1),
    name: z.string(),
    rowCount: count,
    columnCount: count,
    columns,
  })
  .refine((value) => value.columnCount === value.columns.length);

const projectDatabaseSchema = z
  .strictObject({
    id: z.string().min(1),
    engine: z.custom<DatabaseEngineDTO>(isDatabaseEngine),
    schemaVersion: count.max(0xffffffff),
    required: z.boolean(),
    name: z.string().optional(),
    columns,
    rowCount: count.optional(),
    columnCount: count,
    loadFailed: z.boolean(),
  })
  .refine((value) => value.columnCount === value.columns.length);

export type ProjectDatabaseMetadata = z.infer<typeof projectDatabaseSchema>;

export const projectDatabasesSchema = z
  .strictObject({ databases: z.record(z.string().min(1), projectDatabaseSchema) })
  .refine((value) => Object.entries(value.databases).every(([id, database]) => database.id === id));

export const databaseEditStateSchema = z.strictObject({
  canUndo: z.boolean(),
  canRedo: z.boolean(),
  isModified: z.boolean(),
  undoCount: count,
  redoCount: count,
});

export const databaseEmptyResultSchema = z.null();

const rowId = z.string().refine((value) => {
  if (!/^(?:0|-?[1-9][0-9]{0,18})$/.test(value)) return false;
  const integer = BigInt(value);
  return integer >= -9223372036854775808n && integer <= 9223372036854775807n;
});
const cell = z.union([z.string(), z.number(), z.boolean(), z.null()]);
export const databaseRowsSchema = z
  .strictObject({ rows: z.array(z.array(cell)), rowIds: z.array(rowId) })
  .refine(
    (value) =>
      value.rows.length === value.rowIds.length &&
      new Set(value.rowIds).size === value.rowIds.length &&
      value.rows.every((row) => row.length === value.rows[0]?.length),
  );

export const databaseDistributionsSchema = z
  .array(
    z.discriminatedUnion("kind", [
      z.strictObject({
        columnName: z.string(),
        kind: z.literal("numeric"),
        bins: z.array(z.strictObject({ label: z.string(), count })),
      }),
      z.strictObject({
        columnName: z.string(),
        kind: z.literal("string"),
        categories: z.array(z.strictObject({ label: z.string(), value: count })),
        otherCount: count,
      }),
    ]),
  )
  .refine((value) => new Set(value.map((column) => column.columnName)).size === value.length);

export const databaseSourceEntriesSchema = z.array(z.string());

const mutationEnvelope = z.strictObject({ data: z.unknown(), mutation: z.unknown() });

export function parseDatabaseMutationResult<T>(value: unknown, dataSchema: z.ZodType<T>) {
  const envelope = mutationEnvelope.parse(value);
  return {
    data: dataSchema.parse(envelope.data),
    mutation: parseResourceMutationResultDto(envelope.mutation),
  };
}

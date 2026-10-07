import { z } from "zod";
import { isRecord } from "@/shared/types/report/guards";

const count = z.union([z.number().int().nonnegative(), z.string().regex(/^(0|[1-9]\d*)$/)]);
const metric = z.number().nullable();
const category = z.strictObject({
  value: z.union([z.string(), z.number(), z.boolean()]),
  label: z.string().optional(),
  frequency: count,
  proportion: z.number().min(0).max(1),
});
const categories = z
  .custom<Record<string, unknown>>(isRecord)
  .transform((value) => Object.entries(value))
  .pipe(z.array(z.tuple([z.string().regex(/^[1-9]\d*$/), category])))
  .transform((entries) =>
    entries
      .map(([position, entry]) => ({ position: Number(position), ...entry }))
      .sort((left, right) => left.position - right.position),
  )
  .refine((entries) => entries.every((entry, index) => entry.position === index + 1));
const common = {
  position: z.number().int().positive(),
  count,
  missing: count,
};
const column = z.discriminatedUnion("semantic", [
  z.strictObject({
    ...common,
    semantic: z.literal("Numeric"),
    mean: metric,
    std: metric,
    min: metric,
    q25: metric,
    median: metric,
    q75: metric,
    max: metric,
  }),
  z.strictObject({
    ...common,
    semantic: z.enum(["Categorical", "Ordinal", "Binary"]),
    unique: count,
    categories,
  }),
]);
// Validate entries without assigning arbitrary column names to a JavaScript object prototype.
const summaries = z
  .custom<Record<string, unknown>>(isRecord)
  .transform((value) => Object.entries(value))
  .pipe(z.array(z.tuple([z.string(), column])));
const resultSchema = z.strictObject({ columns: summaries });
export type DescriptionColumn = z.infer<typeof column> & { readonly column: string };

export function descriptionColumns(value: unknown): DescriptionColumn[] | null {
  const parsed = resultSchema.safeParse(value);
  if (!parsed.success) return null;
  const columns = parsed.data.columns
    .map(([column, summary]) => ({ column, ...summary }))
    .sort((left, right) => left.position - right.position);
  return columns.every((column, index) => column.position === index + 1) ? columns : null;
}

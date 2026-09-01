export interface DatabaseCellEditInput {
  row: number;
  column: number;
  value: unknown;
}

export type DatabaseCellEditStepOutcome<TError = unknown> =
  | { status: "applied" }
  | { status: "noop" }
  | { status: "failed"; error: TError };

export type DatabaseCellEditBatchOutcome<TError = unknown> =
  | { status: "applied"; appliedCount: number }
  | { status: "noop"; appliedCount: 0 }
  | { status: "cancelled"; appliedCount: number }
  | { status: "failed"; appliedCount: number; error: TError };

interface RunDatabaseCellEditBatchParams<TError> {
  edits: readonly DatabaseCellEditInput[];
  apply: (edit: DatabaseCellEditInput) => Promise<DatabaseCellEditStepOutcome<TError>>;
  isCurrent: () => boolean;
  refresh: () => Promise<void>;
}

export async function runDatabaseCellEditBatch<TError>({
  edits,
  apply,
  isCurrent,
  refresh,
}: RunDatabaseCellEditBatchParams<TError>): Promise<DatabaseCellEditBatchOutcome<TError>> {
  let appliedCount = 0;

  for (const edit of edits) {
    if (!isCurrent()) return { status: "cancelled", appliedCount };
    const outcome = await apply(edit);
    if (outcome.status === "applied") appliedCount += 1;
    if (!isCurrent()) return { status: "cancelled", appliedCount };
    if (outcome.status === "failed") {
      if (appliedCount > 0) await refresh();
      return { status: "failed", appliedCount, error: outcome.error };
    }
  }

  if (!isCurrent()) return { status: "cancelled", appliedCount };
  if (appliedCount === 0) return { status: "noop", appliedCount: 0 };
  await refresh();
  return { status: "applied", appliedCount };
}

import { describe, expect, it, vi } from "vitest";
import { runDatabaseCellEditBatch } from "./databaseCellEditBatch";

const edits = [
  { row: 0, column: 0, value: "first" },
  { row: 0, column: 1, value: "second" },
  { row: 0, column: 2, value: "third" },
];

describe("database cell edit batch", () => {
  it("stops at the first failure and refreshes the applied prefix once", async () => {
    const failure = {
      code: "database_cell_value_invalid",
      incidentId: "incident-batch-1",
    };
    const apply = vi
      .fn()
      .mockResolvedValueOnce({ status: "applied" })
      .mockResolvedValueOnce({ status: "failed", error: failure })
      .mockResolvedValueOnce({ status: "applied" });
    const refresh = vi.fn(async () => undefined);

    await expect(
      runDatabaseCellEditBatch({
        edits,
        apply,
        isCurrent: () => true,
        refresh,
      }),
    ).resolves.toEqual({ status: "failed", appliedCount: 1, error: failure });
    expect(apply).toHaveBeenCalledTimes(2);
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("cancels an obsolete scope without reloading its page", async () => {
    let current = true;
    const apply = vi.fn(async () => {
      current = false;
      return { status: "applied" as const };
    });
    const refresh = vi.fn(async () => undefined);

    await expect(
      runDatabaseCellEditBatch({
        edits,
        apply,
        isCurrent: () => current,
        refresh,
      }),
    ).resolves.toEqual({ status: "cancelled", appliedCount: 1 });
    expect(apply).toHaveBeenCalledTimes(1);
    expect(refresh).not.toHaveBeenCalled();
  });
});

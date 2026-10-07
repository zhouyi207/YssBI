import { afterEach, expect, it, vi } from "vitest";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { uiStore } from "@/features/core/ui/UIStore";
import { DatabaseService } from "@/services/database/databaseService";
import { triggerImportData } from "./useDatabaseManagement";

vi.mock("./dataOperationProgress", () => ({
  runWithDataOperationProgress: (_stage: string, _detail: string, run: () => Promise<unknown>) =>
    run(),
}));
vi.mock("@/services/platform/pathDialog", () => ({ openPathDialog: vi.fn() }));

afterEach(() => {
  for (const modal of uiStore.getState().modals) uiStore.closeModal(modal.id);
  clearProjectLifecycle();
  vi.restoreAllMocks();
});

it("keeps failed connection steps, closes only the import family and ignores a late connection response", async () => {
  startProjectLifecycle("import-project");
  const listTables = vi.spyOn(DatabaseService, "listSqlTables");
  listTables.mockRejectedValueOnce({ code: "CONNECTION_FAILED", details: {}, incidentId: null });
  triggerImportData();
  const root = uiStore.getState().modals[0];
  if (root.type !== "import") throw new Error("missing import");
  await root.options.onSelect("postgres");
  const connection = uiStore.getState().modals[1];
  if (connection.type !== "sqlConnection") throw new Error("missing connection");
  expect(await connection.options.onConnect("postgres://localhost/data")).not.toBeNull();
  expect(uiStore.getState().modals).toEqual([root, connection]);

  listTables.mockResolvedValueOnce(["first", "second"]);
  expect(await connection.options.onConnect("postgres://localhost/data")).toBeNull();
  const table = uiStore.getState().modals[2];
  expect(table.parentId).toBe(connection.id);
  uiStore.closeModal(table.id);
  expect(uiStore.getState().modals).toEqual([root, connection]);

  let resolveTables!: (tables: string[]) => void;
  listTables.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveTables = resolve;
      }),
  );
  const pending = connection.options.onConnect("postgres://localhost/data");
  uiStore.showSettings();
  const settings = uiStore.getState().modals[2];
  const discard = uiStore.confirm(
    { title: "Discard", message: "Discard connection?" },
    connection.id,
  );
  uiStore.closeModal(root.id);
  await expect(discard).resolves.toBe(false);
  expect(uiStore.getState().modals).toEqual([settings]);
  triggerImportData();
  const replacement = uiStore.getState().modals[1];
  resolveTables(["late", "response"]);
  await pending;
  expect(uiStore.getState().modals).toEqual([settings, replacement]);
});

it("settles an explicitly dismissed import confirmation without overriding an accepted decision", async () => {
  startProjectLifecycle("import-project");
  triggerImportData();
  const root = uiStore.getState().modals[0];
  const canceled = uiStore.confirm({ title: "Discard", message: "Discard import?" }, root.id);
  const cancelDialog = uiStore.getState().modals[1];
  uiStore.closeModal(cancelDialog.id);
  await expect(canceled).resolves.toBe(false);
  expect(uiStore.getState().modals).toEqual([root]);

  const accepted = uiStore.confirm({ title: "Discard", message: "Discard import?" }, root.id);
  const acceptDialog = uiStore.getState().modals[1];
  if (acceptDialog.type !== "confirm") throw new Error("missing confirmation");
  acceptDialog.options.onConfirm();
  uiStore.closeModal(acceptDialog.id);
  await expect(accepted).resolves.toBe(true);
  expect(uiStore.getState().modals).toEqual([root]);
});

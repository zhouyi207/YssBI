import { useCallback } from "react";
import { i18n } from "@/app/i18n";
import { useDatabaseStore } from "@/features/core/dataStore";
import { useEditorStore } from "@/features/core/editor";
import { uiStore } from "@/features/core/ui/UIStore";
import { DatabaseService } from "@/services/database/databaseService";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { openPathDialog } from "@/services/platform/pathDialog";
import type { LoadDatabaseResult } from "@/shared/types/domain/database";
import type { DatabaseImportSourceDTO } from "@/shared/types/domain/database";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { logger } from "@/features/application/observability/appLogger";
import { runWithDataOperationProgress } from "./dataOperationProgress";
import { executeDatabaseCreate, executeDatabaseMutation } from "./databaseMutation";
import { databaseRecordFromLoad } from "./databaseRecords";

function showDataOperationMessage(message: string, type: "info" | "warning" = "warning"): void {
  void uiStore.alert({
    title: i18n.t("importModal.title"),
    message,
    closeText: i18n.t("common.close"),
    type,
  });
}

function showDataOperationError(
  error: unknown,
  command: string,
  messageForCode: (code: string) => string,
): void {
  const ipcError = normalizeApplicationIpcError(command, error);
  void uiStore.alert({
    title: i18n.t("common.error"),
    message: messageForCode(ipcError.code),
    closeText: i18n.t("common.close"),
    type: "error",
    incidentId: ipcError.incidentId,
    incidentLabel: i18n.t("common.incidentId"),
  });
}

function logDataOperationFailure(error: unknown, command: string, context: string): void {
  const ipcError = normalizeApplicationIpcError(command, error);
  logger.data.error(
    `${context} failed code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
    "DatabaseManagement",
  );
}

function commitLoadedDatabase(result: LoadDatabaseResult) {
  const store = useDatabaseStore.getState();
  const record = databaseRecordFromLoad(result, store.databases[result.id]);
  if (store.databases[result.id]) store.updateDatabase(result.id, record);
  else store.addDatabase(result.id, record);
}

async function loadSqliteTable(dbPath: string, table: string) {
  const engine: DatabaseImportSourceDTO = {
    sql: {
      engine: "sqlite",
      connectionString: dbPath,
      table,
    },
  };
  const result = await runWithDataOperationProgress(
    i18n.t("dataOperation.importing"),
    i18n.t("dataOperation.importingSqlite", { table }),
    () =>
      executeDatabaseCreate((authority) =>
        DatabaseService.loadDatabase(authority.projectInstanceId, authority.operationId, engine),
      ),
  );
  commitLoadedDatabase(result);
}

type SqlRemoteEngine = "postgres" | "mysql" | "mariadb";

async function loadSqlRemoteTable(
  engine: SqlRemoteEngine,
  connectionString: string,
  table: string,
) {
  const label = engine === "postgres" ? "PostgreSQL" : engine === "mysql" ? "MySQL" : "MariaDB";
  const loadEngine: DatabaseImportSourceDTO = {
    sql: {
      engine: engine === "postgres" ? "postgres" : "mysql",
      connectionString,
      table,
    },
  };
  const result = await runWithDataOperationProgress(
    i18n.t("dataOperation.importing"),
    i18n.t("dataOperation.importingRemote", { label, table }),
    () =>
      executeDatabaseCreate((authority) =>
        DatabaseService.loadDatabase(
          authority.projectInstanceId,
          authority.operationId,
          loadEngine,
        ),
      ),
  );
  commitLoadedDatabase(result);
}

async function loadExcelSheet(filePath: string, sheet: string) {
  const engine: DatabaseImportSourceDTO = { excel: { path: filePath, sheet } };
  const result = await runWithDataOperationProgress(
    i18n.t("dataOperation.importing"),
    i18n.t("dataOperation.importingExcel", { sheet }),
    () =>
      executeDatabaseCreate((authority) =>
        DatabaseService.loadDatabase(authority.projectInstanceId, authority.operationId, engine),
      ),
  );
  commitLoadedDatabase(result);
}

async function loadCsv(path: string) {
  const engine: DatabaseImportSourceDTO = {
    csv: {
      path,
      delimiter: ",",
      hasHeader: true,
      inferSchemaLength: 1000,
    },
  };
  const result = await runWithDataOperationProgress(
    i18n.t("dataOperation.importing"),
    i18n.t("dataOperation.importingCsv"),
    () =>
      executeDatabaseCreate((authority) =>
        DatabaseService.loadDatabase(authority.projectInstanceId, authority.operationId, engine),
      ),
  );
  commitLoadedDatabase(result);
}

/** 触发导入数据弹窗（与菜单栏 Data > Import Data 相同逻辑） */
export function triggerImportData() {
  uiStore.showImportDialog({
    onSelect: async (type) => {
      if (type === "csv") {
        try {
          const result = await openPathDialog({
            multiple: false,
            filters: [{ name: "CSV File", extensions: ["csv"] }],
          });
          if (!result.ok) throw new Error(result.failure.code);
          const selected = result.value;
          if (selected && !Array.isArray(selected)) {
            await loadCsv(selected);
          }
        } catch (error) {
          logDataOperationFailure(error, "load_database", "CSV import");
          showDataOperationError(error, "load_database", (code) =>
            i18n.t("dataOperation.importFailed", { error: code }),
          );
        }
      } else if (type === "sqlite") {
        try {
          const result = await openPathDialog({
            multiple: false,
            filters: [
              { name: "SQLite Database", extensions: ["db", "sqlite", "sqlite3"] },
              { name: "All Files", extensions: ["*"] },
            ],
          });
          if (!result.ok) throw new Error(result.failure.code);
          const selected = result.value;
          if (selected && !Array.isArray(selected)) {
            const tables = await runWithDataOperationProgress(
              i18n.t("dataOperation.reading"),
              i18n.t("dataOperation.readingSqlite"),
              () => DatabaseService.listSqliteTables(selected),
            );
            if (tables.length === 0) {
              showDataOperationMessage(i18n.t("dataOperation.noSqliteTables"));
              return;
            }
            if (tables.length === 1) {
              await loadSqliteTable(selected, tables[0]);
            } else {
              uiStore.showSqliteTableSelectDialog({
                dbPath: selected,
                tables,
                onSelect: (table) => {
                  loadSqliteTable(selected, table).catch((error) => {
                    logDataOperationFailure(error, "load_database", "SQLite table load");
                    showDataOperationError(error, "load_database", (code) =>
                      i18n.t("dataOperation.importFailed", { error: code }),
                    );
                  });
                },
              });
            }
          }
        } catch (error) {
          logDataOperationFailure(error, "list_sqlite_tables", "SQLite import");
          showDataOperationError(error, "list_sqlite_tables", (code) =>
            i18n.t("dataOperation.importFailed", { error: code }),
          );
        }
      } else if (["postgres", "mysql", "mariadb"].includes(type)) {
        const engine = type as SqlRemoteEngine;
        const label =
          engine === "postgres" ? "PostgreSQL" : engine === "mysql" ? "MySQL" : "MariaDB";
        uiStore.showSqlConnectionDialog({
          engine,
          onConnect: async (connectionString) => {
            try {
              const tables = await runWithDataOperationProgress(
                i18n.t("dataOperation.reading"),
                i18n.t("dataOperation.readingRemote", { label }),
                () => DatabaseService.listSqlTables(engine, connectionString),
              );
              if (tables.length === 0) {
                showDataOperationMessage(i18n.t("dataOperation.noRemoteTables"));
                return;
              }
              if (tables.length === 1) {
                await loadSqlRemoteTable(engine, connectionString, tables[0]);
              } else {
                uiStore.showSqlRemoteTableSelectDialog({
                  connectionString,
                  engine,
                  tables,
                  onSelect: (table) => {
                    loadSqlRemoteTable(engine, connectionString, table).catch((error) => {
                      logDataOperationFailure(error, "load_database", `${label} table load`);
                      showDataOperationError(error, "load_database", (code) =>
                        i18n.t("dataOperation.importFailed", { error: code }),
                      );
                    });
                  },
                });
              }
            } catch (error) {
              logDataOperationFailure(error, "list_sql_tables", `${label} table listing`);
              showDataOperationError(error, "list_sql_tables", (code) =>
                i18n.t("dataOperation.connectFailed", { label, error: code }),
              );
            }
          },
        });
      } else if (type === "xlsx") {
        try {
          const result = await openPathDialog({
            multiple: false,
            filters: [
              { name: "Excel File", extensions: ["xlsx", "xls"] },
              { name: "All Files", extensions: ["*"] },
            ],
          });
          if (!result.ok) throw new Error(result.failure.code);
          const selected = result.value;
          if (selected && !Array.isArray(selected)) {
            const sheets = await runWithDataOperationProgress(
              i18n.t("dataOperation.reading"),
              i18n.t("dataOperation.readingExcel"),
              () => DatabaseService.listExcelSheets(selected),
            );
            if (sheets.length === 0) {
              showDataOperationMessage(i18n.t("dataOperation.noExcelSheets"));
              return;
            }
            if (sheets.length === 1) {
              await loadExcelSheet(selected, sheets[0]);
            } else {
              uiStore.showExcelSheetSelectDialog({
                filePath: selected,
                sheets,
                onSelect: (sheet) => {
                  loadExcelSheet(selected, sheet).catch((error) => {
                    logDataOperationFailure(error, "load_database", "Excel sheet load");
                    showDataOperationError(error, "load_database", (code) =>
                      i18n.t("dataOperation.importFailed", { error: code }),
                    );
                  });
                },
              });
            }
          }
        } catch (error) {
          logDataOperationFailure(error, "list_excel_sheets", "Excel import");
          showDataOperationError(error, "list_excel_sheets", (code) =>
            i18n.t("dataOperation.importFailed", { error: code }),
          );
        }
      } else {
        showDataOperationMessage(
          i18n.t("dataOperation.comingSoon", { type: String(type).toUpperCase() }),
          "info",
        );
      }
    },
  });
}

// database
export function useDatabaseManagement() {
  const detailFocus = useEditorStore((s) => s.detailFocus);
  const clearDetailFocus = useEditorStore((s) => s.clearDetailFocus);

  const updateDataFrame = useCallback((id: string, data: Partial<DatabaseRecord>) => {
    useDatabaseStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback(
    async (id: string) => {
      const previous = useDatabaseStore.getState().databases[id];
      if (!previous) return;

      try {
        await runWithDataOperationProgress(
          i18n.t("dataOperation.deleting"),
          String(previous.name ?? id),
          () =>
            executeDatabaseMutation(id, (authority) =>
              DatabaseService.deleteDatabase(
                authority.projectInstanceId,
                authority.operationId,
                authority.expectedRevision,
                id,
              ),
            ),
        );
        if (detailFocus?.kind === "data" && detailFocus.id === id) {
          clearDetailFocus();
        }
      } catch (e) {
        logDataOperationFailure(e, "delete_database", "Database deletion");
        showDataOperationError(e, "delete_database", (code) =>
          i18n.t("dataOperation.deleteFailed", { error: code }),
        );
      }
    },
    [detailFocus, clearDetailFocus],
  );

  const renameDataFrame = useCallback(async (id: string, name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;

    try {
      await executeDatabaseMutation(id, (authority) =>
        DatabaseService.renameDatabase(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          id,
          trimmed,
        ),
      );
    } catch (e) {
      logDataOperationFailure(e, "rename_database", "Database rename");
      showDataOperationError(e, "rename_database", (code) =>
        i18n.t("dataOperation.renameFailed", { error: code }),
      );
    }
  }, []);

  return {
    triggerImportData,
    updateDataFrame,
    deleteDataFrame,
    renameDataFrame,
  };
}

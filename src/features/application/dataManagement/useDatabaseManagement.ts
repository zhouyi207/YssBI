import { useCallback } from "react";
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
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

function showDataOperationError(error: unknown, messageForCode: (code: string) => string): void {
  const ipcError = normalizeApplicationIpcError(error);
  void uiStore.alert({
    title: i18n.t("common.error"),
    message: messageForCode(ipcError.code),
    closeText: i18n.t("common.close"),
    type: "error",
    incidentId: ipcError.incidentId,
    incidentLabel: i18n.t("common.incidentId"),
  });
}

function logDataOperationFailure(error: unknown, context: string): void {
  const ipcError = normalizeApplicationIpcError(error);
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

async function loadSqliteTable(project: ProjectIdentitySnapshot, dbPath: string, table: string) {
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
      executeDatabaseCreate((authority) => {
        assertCurrentProjectIdentity(project);
        return DatabaseService.loadDatabase(
          authority.projectInstanceId,
          authority.operationId,
          engine,
        );
      }),
  );
  assertCurrentProjectIdentity(project);
  commitLoadedDatabase(result);
}

type SqlRemoteEngine = "postgres" | "mysql" | "mariadb";

async function loadSqlRemoteTable(
  project: ProjectIdentitySnapshot,
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
      executeDatabaseCreate((authority) => {
        assertCurrentProjectIdentity(project);
        return DatabaseService.loadDatabase(
          authority.projectInstanceId,
          authority.operationId,
          loadEngine,
        );
      }),
  );
  assertCurrentProjectIdentity(project);
  commitLoadedDatabase(result);
}

async function loadExcelSheet(project: ProjectIdentitySnapshot, filePath: string, sheet: string) {
  const engine: DatabaseImportSourceDTO = { excel: { path: filePath, sheet } };
  const result = await runWithDataOperationProgress(
    i18n.t("dataOperation.importing"),
    i18n.t("dataOperation.importingExcel", { sheet }),
    () =>
      executeDatabaseCreate((authority) => {
        assertCurrentProjectIdentity(project);
        return DatabaseService.loadDatabase(
          authority.projectInstanceId,
          authority.operationId,
          engine,
        );
      }),
  );
  assertCurrentProjectIdentity(project);
  commitLoadedDatabase(result);
}

async function loadCsv(project: ProjectIdentitySnapshot, path: string) {
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
      executeDatabaseCreate((authority) => {
        assertCurrentProjectIdentity(project);
        return DatabaseService.loadDatabase(
          authority.projectInstanceId,
          authority.operationId,
          engine,
        );
      }),
  );
  assertCurrentProjectIdentity(project);
  commitLoadedDatabase(result);
}

/** 触发导入数据弹窗（与菜单栏 Data > Import Data 相同逻辑） */
export function triggerImportData() {
  if (uiStore.getState().modals.some((modal) => modal.type === "import")) return;
  const project = captureProjectIdentity();
  const current = () =>
    isCurrentProjectIdentity(project) &&
    uiStore.getState().modals.some((modal) => modal.id === importModalId);
  let busy = false;

  const importSelected = async (load: () => Promise<void>): Promise<string | null> => {
    if (!current() || busy) return null;
    busy = true;
    try {
      await load();
      if (current()) uiStore.closeModal(importModalId);
      return null;
    } catch (error) {
      if (!current()) return null;
      logDataOperationFailure(error, "Data import");
      const failure = normalizeApplicationIpcError(error);
      return i18n.t("dataOperation.importFailed", { error: failure.code });
    } finally {
      busy = false;
    }
  };

  const importModalId = uiStore.showImportDialog({
    onImportSample: async (sampleId, version) => {
      if (!current()) return;
      try {
        const result = await executeDatabaseCreate((authority) =>
          DatabaseService.importSampleDataset(
            authority.projectInstanceId,
            authority.operationId,
            sampleId,
            version,
          ),
        );
        if (!current()) return;
        commitLoadedDatabase(result);
        uiStore.closeModal(importModalId);
      } catch (error) {
        logDataOperationFailure(error, "Sample import");
        throw error;
      }
    },
    onSelect: async (type) => {
      if (!current() || busy) return;
      if (type === "postgres" || type === "mysql" || type === "mariadb") {
        const engine = type;
        const label =
          engine === "postgres" ? "PostgreSQL" : engine === "mysql" ? "MySQL" : "MariaDB";
        const connectionId = uiStore.showSqlConnectionDialog(
          {
            engine,
            onConnect: async (connectionString) => {
              const connectionCurrent = () =>
                current() && uiStore.getState().modals.some((modal) => modal.id === connectionId);
              if (!connectionCurrent()) return null;
              try {
                const tables = await runWithDataOperationProgress(
                  i18n.t("dataOperation.reading"),
                  i18n.t("dataOperation.readingRemote", { label }),
                  () => DatabaseService.listSqlTables(engine, connectionString),
                );
                if (!connectionCurrent()) return null;
                if (!tables.length) return i18n.t("dataOperation.noRemoteTables");
                if (tables.length === 1)
                  return importSelected(() =>
                    loadSqlRemoteTable(project, engine, connectionString, tables[0]),
                  );
                uiStore.showSqlRemoteTableSelectDialog(
                  {
                    connectionString,
                    engine,
                    tables,
                    onSelect: (table) =>
                      importSelected(() =>
                        loadSqlRemoteTable(project, engine, connectionString, table),
                      ),
                  },
                  connectionId!,
                );
                return null;
              } catch (error) {
                if (!connectionCurrent()) return null;
                logDataOperationFailure(error, "Remote table listing");
                const failure = normalizeApplicationIpcError(error);
                return i18n.t("dataOperation.connectFailed", { label, error: failure.code });
              }
            },
          },
          importModalId,
        );
        return;
      }
      if (type !== "csv" && type !== "sqlite" && type !== "xlsx") {
        showDataOperationMessage(
          i18n.t("dataOperation.comingSoon", { type: type.toUpperCase() }),
          "info",
        );
        return;
      }
      busy = true;
      try {
        const selection = await openPathDialog({
          multiple: false,
          filters:
            type === "csv"
              ? [{ name: "CSV File", extensions: ["csv"] }]
              : type === "sqlite"
                ? [
                    { name: "SQLite Database", extensions: ["db", "sqlite", "sqlite3"] },
                    { name: "All Files", extensions: ["*"] },
                  ]
                : [
                    { name: "Excel File", extensions: ["xlsx", "xls"] },
                    { name: "All Files", extensions: ["*"] },
                  ],
        });
        if (!current()) return;
        if (!selection.ok) throw new Error(selection.failure.code);
        const path = selection.value;
        if (!path || Array.isArray(path)) return;
        if (type === "csv") {
          await loadCsv(project, path);
          if (current()) uiStore.closeModal(importModalId);
          return;
        }
        const entries = await runWithDataOperationProgress(
          i18n.t("dataOperation.reading"),
          i18n.t(type === "sqlite" ? "dataOperation.readingSqlite" : "dataOperation.readingExcel"),
          () =>
            type === "sqlite"
              ? DatabaseService.listSqliteTables(path)
              : DatabaseService.listExcelSheets(path),
        );
        if (!current()) return;
        if (!entries.length) {
          showDataOperationMessage(
            i18n.t(
              type === "sqlite" ? "dataOperation.noSqliteTables" : "dataOperation.noExcelSheets",
            ),
          );
          return;
        }
        const load = (entry: string) =>
          type === "sqlite"
            ? loadSqliteTable(project, path, entry)
            : loadExcelSheet(project, path, entry);
        if (entries.length === 1) {
          await load(entries[0]);
          if (current()) uiStore.closeModal(importModalId);
        } else if (type === "sqlite") {
          uiStore.showSqliteTableSelectDialog(
            {
              dbPath: path,
              tables: entries,
              onSelect: (table) => importSelected(() => load(table)),
            },
            importModalId,
          );
        } else {
          uiStore.showExcelSheetSelectDialog(
            {
              filePath: path,
              sheets: entries,
              onSelect: (sheet) => importSelected(() => load(sheet)),
            },
            importModalId,
          );
        }
      } catch (error) {
        if (!current()) return;
        logDataOperationFailure(error, "File import");
        showDataOperationError(error, (code) =>
          i18n.t("dataOperation.importFailed", { error: code }),
        );
      } finally {
        busy = false;
      }
    },
  });
}

// database
export function useDatabaseManagement() {
  const updateDataFrame = useCallback((id: string, data: Partial<DatabaseRecord>) => {
    useDatabaseStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback(async (id: string) => {
    if (!useDatabaseStore.getState().databases[id]) return;

    try {
      await executeDatabaseMutation(id, (authority) =>
        DatabaseService.deleteDatabase(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          id,
        ),
      );
      const editor = useEditorStore.getState();
      if (editor.detailFocus?.kind === "data" && editor.detailFocus.id === id) {
        editor.clearDetailFocus();
      }
    } catch (e) {
      logDataOperationFailure(e, "Database deletion");
      showDataOperationError(e, (code) => i18n.t("dataOperation.deleteFailed", { error: code }));
    }
  }, []);

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
      logDataOperationFailure(e, "Database rename");
      showDataOperationError(e, (code) => i18n.t("dataOperation.renameFailed", { error: code }));
    }
  }, []);

  return {
    triggerImportData,
    updateDataFrame,
    deleteDataFrame,
    renameDataFrame,
  };
}

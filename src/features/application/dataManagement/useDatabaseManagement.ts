import { useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { i18n } from '@/app/i18n';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { uiStore } from '@/features/core/ui/UIStore';
import { DatabaseService } from '@/services/database/databaseService';
import { normalizeIpcError } from '@/services/ipc';
import type { LoadDatabaseResult } from '@/shared/types/dto/database';
import { databaseRecordFromLoad } from '@/shared/types/dto/database';
import type { DatabaseRecord, LoadDatabaseEngineSpec } from '@/shared/types/dto/database';
import { logger } from '@/utils/appLogger';
import { runWithDataOperationProgress } from './dataOperationProgress';
import { executeDatabaseCreate, executeDatabaseMutation } from './databaseMutation';

function showDataOperationMessage(message: string, type: 'info' | 'warning' = 'warning'): void {
  void uiStore.alert({
    title: i18n.t('importModal.title'),
    message,
    closeText: i18n.t('common.close'),
    type,
  });
}

function showDataOperationError(
  error: unknown,
  command: string,
  messageForCode: (code: string) => string,
): void {
  const ipcError = normalizeIpcError(command, error);
  void uiStore.alert({
    title: i18n.t('common.error'),
    message: messageForCode(ipcError.code),
    closeText: i18n.t('common.close'),
    type: 'error',
    incidentId: ipcError.incidentId,
    incidentLabel: i18n.t('common.incidentId'),
  });
}

function commitLoadedDatabase(result: LoadDatabaseResult, engine: LoadDatabaseEngineSpec) {
  const store = useDatabaseStore.getState();
  const record = databaseRecordFromLoad(result, engine);
  if (store.databases[result.id]) store.updateDatabase(result.id, record);
  else store.addDatabase(result.id, record);
}

async function loadSqliteTable(dbPath: string, table: string) {
  const engine: LoadDatabaseEngineSpec = {
    sql: {
      engine: { sqlite: { autoCreate: false } },
      connectionString: dbPath,
      table,
    },
  };
  const result = await runWithDataOperationProgress(
    i18n.t('dataOperation.importing'),
    i18n.t('dataOperation.importingSqlite', { table }),
    () => executeDatabaseCreate((authority) => DatabaseService.loadDatabase(
      authority.projectInstanceId,
      authority.operationId,
      engine,
    )),
  );
  commitLoadedDatabase(result, engine);
}

type SqlRemoteEngine = 'postgres' | 'mysql' | 'mariadb';

async function loadSqlRemoteTable(engine: SqlRemoteEngine, connectionString: string, table: string) {
  const label =
    engine === 'postgres' ? 'PostgreSQL' : engine === 'mysql' ? 'MySQL' : 'MariaDB';
  const loadEngine: LoadDatabaseEngineSpec =
    engine === 'postgres'
      ? { sql: { engine: { postgres: { ssl: true } }, connectionString, table } }
      : { sql: { engine: { mysql: { charset: 'utf8mb4' } }, connectionString, table } };
  const result = await runWithDataOperationProgress(
    i18n.t('dataOperation.importing'),
    i18n.t('dataOperation.importingRemote', { label, table }),
    () => executeDatabaseCreate((authority) => DatabaseService.loadDatabase(
      authority.projectInstanceId,
      authority.operationId,
      loadEngine,
    )),
  );
  commitLoadedDatabase(result, loadEngine);
}

async function loadExcelSheet(filePath: string, sheet: string) {
  const engine: LoadDatabaseEngineSpec = { excel: { path: filePath, sheet } };
  const result = await runWithDataOperationProgress(
    i18n.t('dataOperation.importing'),
    i18n.t('dataOperation.importingExcel', { sheet }),
    () => executeDatabaseCreate((authority) => DatabaseService.loadDatabase(
      authority.projectInstanceId,
      authority.operationId,
      engine,
    )),
  );
  commitLoadedDatabase(result, engine);
}

async function loadCsv(path: string) {
  const engine: LoadDatabaseEngineSpec = {
    csv: {
      path,
      delimiter: ',',
      hasHeader: true,
      inferSchemaLength: 1000,
    },
  };
  const result = await runWithDataOperationProgress(
    i18n.t('dataOperation.importing'),
    i18n.t('dataOperation.importingCsv'),
    () => executeDatabaseCreate((authority) => DatabaseService.loadDatabase(
      authority.projectInstanceId,
      authority.operationId,
      engine,
    )),
  );
  commitLoadedDatabase(result, engine);
}

/** 触发导入数据弹窗（与菜单栏 Data > Import Data 相同逻辑） */
export function triggerImportData() {
  uiStore.showImportDialog({
    onSelect: async (type) => {
      if (type === 'csv') {
        try {
          const selected = await open({
            multiple: false,
            filters: [{ name: 'CSV File', extensions: ['csv'] }],
          });
          if (selected && !Array.isArray(selected)) {
            await loadCsv(selected);
          }
        } catch (error) {
          logger.data.error('Failed to import CSV: ' + String(error), 'DatabaseManagement');
          showDataOperationError(error, 'load_database', (code) =>
            i18n.t('dataOperation.importFailed', { error: code }));
        }
      } else if (type === 'sqlite') {
        try {
          const selected = await open({
            multiple: false,
            filters: [
              { name: 'SQLite Database', extensions: ['db', 'sqlite', 'sqlite3'] },
              { name: 'All Files', extensions: ['*'] },
            ],
          });
          if (selected && !Array.isArray(selected)) {
            const tables = await runWithDataOperationProgress(
              i18n.t('dataOperation.reading'),
              i18n.t('dataOperation.readingSqlite'),
              () => DatabaseService.listSqliteTables(selected),
            );
            if (tables.length === 0) {
              showDataOperationMessage(i18n.t('dataOperation.noSqliteTables'));
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
                    logger.data.error('Failed to load SQLite table: ' + String(error), 'DatabaseManagement');
                    showDataOperationError(error, 'load_database', (code) =>
                      i18n.t('dataOperation.importFailed', { error: code }));
                  });
                },
              });
            }
          }
        } catch (error) {
          logger.data.error('Failed to import SQLite: ' + String(error), 'DatabaseManagement');
          showDataOperationError(error, 'list_sqlite_tables', (code) =>
            i18n.t('dataOperation.importFailed', { error: code }));
        }
      } else if (['postgres', 'mysql', 'mariadb'].includes(type)) {
        const engine = type as SqlRemoteEngine;
        const label = engine === 'postgres' ? 'PostgreSQL' : engine === 'mysql' ? 'MySQL' : 'MariaDB';
        uiStore.showSqlConnectionDialog({
          engine,
          onConnect: async (connectionString) => {
            try {
              const tables = await runWithDataOperationProgress(
                i18n.t('dataOperation.reading'),
                i18n.t('dataOperation.readingRemote', { label }),
                () => DatabaseService.listSqlTables(engine, connectionString),
              );
              if (tables.length === 0) {
                showDataOperationMessage(i18n.t('dataOperation.noRemoteTables'));
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
                      logger.data.error(`Failed to load ${label} table: ${String(error)}`, 'DatabaseManagement');
                      showDataOperationError(error, 'load_database', (code) =>
                        i18n.t('dataOperation.importFailed', { error: code }));
                    });
                  },
                });
              }
            } catch (error) {
              logger.data.error(`Failed to list ${label} tables: ${String(error)}`, 'DatabaseManagement');
              showDataOperationError(error, 'list_sql_tables', (code) =>
                i18n.t('dataOperation.connectFailed', { label, error: code }));
            }
          },
        });
      } else if (type === 'xlsx') {
        try {
          const selected = await open({
            multiple: false,
            filters: [
              { name: 'Excel File', extensions: ['xlsx', 'xls'] },
              { name: 'All Files', extensions: ['*'] },
            ],
          });
          if (selected && !Array.isArray(selected)) {
            const sheets = await runWithDataOperationProgress(
              i18n.t('dataOperation.reading'),
              i18n.t('dataOperation.readingExcel'),
              () => DatabaseService.listExcelSheets(selected),
            );
            if (sheets.length === 0) {
              showDataOperationMessage(i18n.t('dataOperation.noExcelSheets'));
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
                    logger.data.error('Failed to load Excel sheet: ' + String(error), 'DatabaseManagement');
                    showDataOperationError(error, 'load_database', (code) =>
                      i18n.t('dataOperation.importFailed', { error: code }));
                  });
                },
              });
            }
          }
        } catch (error) {
          logger.data.error('Failed to import Excel: ' + String(error), 'DatabaseManagement');
          showDataOperationError(error, 'list_excel_sheets', (code) =>
            i18n.t('dataOperation.importFailed', { error: code }));
        }
      } else {
        showDataOperationMessage(
          i18n.t('dataOperation.comingSoon', { type: String(type).toUpperCase() }),
          'info',
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

  const deleteDataFrame = useCallback(async (id: string) => {
    const previous = useDatabaseStore.getState().databases[id];
    if (!previous) return;

    try {
      await runWithDataOperationProgress(
        i18n.t('dataOperation.deleting'),
        String(previous.name ?? id),
        () => executeDatabaseMutation(id, (authority) => DatabaseService.deleteDatabase(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          id,
        )),
      );
      if (detailFocus?.kind === 'data' && detailFocus.id === id) {
        clearDetailFocus();
      }
    } catch (e) {
      logger.data.warn('deleteDatabase backend failed: ' + String(e), 'DatabaseManagement');
      showDataOperationError(e, 'delete_database', (code) =>
        i18n.t('dataOperation.deleteFailed', { error: code }));
    }
  }, [detailFocus, clearDetailFocus]);

  const renameDataFrame = useCallback(async (id: string, name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;

    try {
      await executeDatabaseMutation(id, (authority) => DatabaseService.renameDatabase(
        authority.projectInstanceId,
        authority.operationId,
        authority.expectedRevision,
        id,
        trimmed,
      ));
    } catch (e) {
      logger.data.warn('renameDatabase backend failed: ' + String(e), 'DatabaseManagement');
      showDataOperationError(e, 'rename_database', (code) =>
        i18n.t('dataOperation.renameFailed', { error: code }));
    }
  }, []);

  return {
    triggerImportData,
    updateDataFrame,
    deleteDataFrame,
    renameDataFrame,
  };
}

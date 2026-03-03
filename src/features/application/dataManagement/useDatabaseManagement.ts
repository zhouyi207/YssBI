import { useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { uiStore } from '@/features/core/ui/UIStore';
import { DatabaseService } from '@/services/database/databaseService';
import { logger } from '@/utils/appLogger';

async function loadSqliteTable(dbPath: string, table: string) {
  uiStore.showToast('正在从 SQLite 导入数据...', 'info');
  const result = await DatabaseService.loadDatabase({
    sql: {
      engine: { sqlite: { autoCreate: false } },
      connectionString: dbPath,
      table,
    },
  });
  useDatabaseStore.getState().addDatabase(result.id, { ...result });
  uiStore.showToast(`SQLite 表 "${table}" 导入成功: ${result.rowCount} 行`, 'success');
}

type SqlRemoteEngine = 'postgres' | 'mysql' | 'mariadb';

async function loadSqlRemoteTable(engine: SqlRemoteEngine, connectionString: string, table: string) {
  const label = engine === 'postgres' ? 'PostgreSQL' : engine === 'mysql' ? 'MySQL' : 'MariaDB';
  uiStore.showToast(`正在从 ${label} 导入数据...`, 'info');
  const engineSpec =
    engine === 'postgres'
      ? { postgres: { ssl: true } }
      : { mysql: { charset: 'utf8mb4' } };
  const result = await DatabaseService.loadDatabase({
    sql: {
      engine: engineSpec,
      connectionString,
      table,
    },
  });
  useDatabaseStore.getState().addDatabase(result.id, { ...result });
  uiStore.showToast(`${label} 表 "${table}" 导入成功: ${result.rowCount} 行`, 'success');
}

async function loadExcelSheet(filePath: string, sheet: string) {
  uiStore.showToast('正在从 Excel 导入数据...', 'info');
  const result = await DatabaseService.loadDatabase({
    excel: { path: filePath, sheet },
  });
  useDatabaseStore.getState().addDatabase(result.id, { ...result });
  uiStore.showToast(`Excel Sheet "${sheet}" 导入成功: ${result.rowCount} 行`, 'success');
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
            uiStore.showToast('正在从 CSV 导入数据...', 'info');
            const result = await DatabaseService.loadDatabase({
              csv: {
                path: selected,
                delimiter: ',',
                hasHeader: true,
                inferSchemaLength: 1000,
              },
            });
            useDatabaseStore.getState().addDatabase(result.id, { ...result });
            uiStore.showToast(`CSV 数据导入成功: ${result.rowCount} 行`, 'success');
          }
        } catch (error) {
          logger.data.error('Failed to import CSV: ' + String(error), 'DatabaseManagement');
          uiStore.showToast(`CSV 导入失败: ${error}`, 'error');
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
            const tables = await DatabaseService.listSqliteTables(selected);
            if (tables.length === 0) {
              uiStore.showToast('该数据库中没有用户表', 'warning');
              return;
            }
            if (tables.length === 1) {
              await loadSqliteTable(selected, tables[0]);
            } else {
              uiStore.showSqliteTableSelectDialog({
                dbPath: selected,
                tables,
                onSelect: (table) => {
                  loadSqliteTable(selected, table).catch((e) => {
                    logger.data.error('Failed to load SQLite table: ' + String(e), 'DatabaseManagement');
                    uiStore.showToast(`导入失败: ${e}`, 'error');
                  });
                },
              });
            }
          }
        } catch (error) {
          logger.data.error('Failed to import SQLite: ' + String(error), 'DatabaseManagement');
          uiStore.showToast(`SQLite 导入失败: ${error}`, 'error');
        }
      } else if (['postgres', 'mysql', 'mariadb'].includes(type)) {
        const engine = type as SqlRemoteEngine;
        const label = engine === 'postgres' ? 'PostgreSQL' : engine === 'mysql' ? 'MySQL' : 'MariaDB';
        uiStore.showSqlConnectionDialog({
          engine,
          onConnect: async (connectionString) => {
            try {
              const tables = await DatabaseService.listSqlTables(engine, connectionString);
              if (tables.length === 0) {
                uiStore.showToast('该数据库中未找到用户表', 'warning');
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
                    loadSqlRemoteTable(engine, connectionString, table).catch((e) => {
                      logger.data.error(`Failed to load ${label} table: ${String(e)}`, 'DatabaseManagement');
                      uiStore.showToast(`导入失败: ${e}`, 'error');
                    });
                  },
                });
              }
            } catch (error) {
              logger.data.error(`Failed to list ${label} tables: ${String(error)}`, 'DatabaseManagement');
              uiStore.showToast(`${label} 连接失败: ${error}`, 'error');
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
            const sheets = await DatabaseService.listExcelSheets(selected);
            if (sheets.length === 0) {
              uiStore.showToast('该 Excel 文件中没有 Sheet', 'warning');
              return;
            }
            if (sheets.length === 1) {
              await loadExcelSheet(selected, sheets[0]);
            } else {
              uiStore.showExcelSheetSelectDialog({
                filePath: selected,
                sheets,
                onSelect: (sheet) => {
                  loadExcelSheet(selected, sheet).catch((e) => {
                    logger.data.error('Failed to load Excel sheet: ' + String(e), 'DatabaseManagement');
                    uiStore.showToast(`导入失败: ${e}`, 'error');
                  });
                },
              });
            }
          }
        } catch (error) {
          logger.data.error('Failed to import Excel: ' + String(error), 'DatabaseManagement');
          uiStore.showToast(`Excel 导入失败: ${error}`, 'error');
        }
      } else {
        uiStore.showToast(`${String(type).toUpperCase()} 导入功能开发中...`, 'warning');
      }
    },
  });
}

// database
export function useDatabaseManagement() {
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);

  const updateDataFrame = useCallback((id: string, data: any) => {
    useDatabaseStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useDatabaseStore.getState().deleteDatabase(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
    DatabaseService.deleteDatabase(id).catch((e) =>
      logger.data.warn('deleteDatabase backend failed: ' + String(e), 'DatabaseManagement')
    );
  }, [selectedItemId, setSelectedInfo]);

  return {
    triggerImportData,
    updateDataFrame,
    deleteDataFrame,
  };
}

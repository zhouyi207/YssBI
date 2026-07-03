import { useRef, useCallback } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { DatabaseService } from '@/services/database/databaseService';
import { invalidateWorksheetPreviewCacheForDatabase } from '@/services/worksheet/worksheetPreviewCache';
import { useDatabaseStore, useEditStateStore } from '@/features/core/dataStore';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { EMPTY_EDIT_STATE } from '@/features/core/dataStore/editStateStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';

export interface ColumnMeta { name: string; type: string; }

interface UseEditActionsParams {
  selectedDfId: string | null;
  columns: ColumnMeta[];
  loadedRows: any[][];
  loadedRowIds: number[];
  rowOffset: number;
  reloadAllData: () => Promise<void>;
}

export function useEditActions({
  selectedDfId,
  columns,
  loadedRows,
  loadedRowIds,
  rowOffset,
  reloadAllData,
}: UseEditActionsParams) {
  const editStateByDatabase = useEditStateStore(s => s.editStateByDatabase);
  const commitInFlightRef = useRef(false);

  const currentEditState: EditState = selectedDfId
    ? (editStateByDatabase[selectedDfId] ?? EMPTY_EDIT_STATE)
    : EMPTY_EDIT_STATE;

  const handleEditResult = useCallback(async (editState: EditState) => {
    if (!selectedDfId) return;
    invalidateWorksheetPreviewCacheForDatabase(selectedDfId);
    useEditStateStore.getState().updateEditState(selectedDfId, editState);
    await reloadAllData();
  }, [selectedDfId, reloadAllData]);

  const commitCellValue = useCallback(async (row: number, col: number, value: unknown) => {
    if (!selectedDfId) return;
    if (commitInFlightRef.current) return;
    const globalRow = rowOffset + row;
    const colName = columns[col]?.name;
    if (!colName) return;

    const oldVal = loadedRows[row]?.[col];
    const oldStr = oldVal === null || oldVal === undefined ? '' : String(oldVal);
    const nextStr = value === null || value === undefined ? '' : String(value);
    if (nextStr === oldStr) return;

    try {
      commitInFlightRef.current = true;
      let parsed: unknown = value;
      if (nextStr === '') parsed = null;
      else if (typeof value === 'string' && !isNaN(Number(value)) && value.trim() !== '') parsed = Number(value);
      const rowId = loadedRowIds[row];
      const es = await DatabaseService.editCell(
        selectedDfId,
        globalRow,
        colName,
        parsed,
        rowId,
      );
      await handleEditResult(es);
    } catch (e) {
      const msg = String(e);
      logger.data.error('editCell failed: ' + msg, 'DatabaseEditorWindow');
      uiStore.showToast(msg, 'error', 5000);
    } finally {
      commitInFlightRef.current = false;
    }
  }, [selectedDfId, columns, loadedRows, loadedRowIds, rowOffset, handleEditResult]);

  const handleUndo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canUndo) return;
    try {
      const es = await DatabaseService.undoEdit(selectedDfId);
      await handleEditResult(es);
    } catch (e) { logger.data.error('undo failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, currentEditState.canUndo, handleEditResult]);

  const handleRedo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canRedo) return;
    try {
      const es = await DatabaseService.redoEdit(selectedDfId);
      await handleEditResult(es);
    } catch (e) { logger.data.error('redo failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, currentEditState.canRedo, handleEditResult]);

  const handleSave = useCallback(async () => {
    if (!selectedDfId || !currentEditState.isModified) return;
    try {
      const es = await DatabaseService.saveDatabaseChanges(selectedDfId);
      await handleEditResult(es);
      uiStore.showToast('数据已保存到项目', 'success', 3000);
    } catch (e) {
      const msg = String(e);
      logger.data.error('save changes failed: ' + msg, 'DatabaseEditorWindow');
      uiStore.showToast(msg, 'error', 5000);
    }
  }, [selectedDfId, currentEditState.isModified, handleEditResult]);

  const handleExport = useCallback(async () => {
    if (!selectedDfId) return;
    try {
      const filePath = await save({
        title: 'Export Data',
        filters: [
          { name: 'CSV', extensions: ['csv'] },
          { name: 'Parquet', extensions: ['parquet'] },
        ],
      });
      if (!filePath) return;
      const fmt = filePath.endsWith('.parquet') ? 'parquet' : 'csv';
      await DatabaseService.exportDatabase(selectedDfId, filePath, fmt);
    } catch (e) { logger.data.error('export failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId]);

  const handleAddRow = useCallback(async (index?: number) => {
    if (!selectedDfId) return;
    try {
      const globalIndex = index === undefined ? undefined : rowOffset + index;
      const es = await DatabaseService.addRow(selectedDfId, globalIndex);
      await handleEditResult(es);
    } catch (e) { logger.data.error('addRow failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, rowOffset, handleEditResult]);

  const handleDeleteRow = useCallback(async (indices: number[]) => {
    if (!selectedDfId || indices.length === 0) return;
    try {
      const globalIndices = indices.map((index) => rowOffset + index);
      const rowIds = indices
        .map((index) => loadedRowIds[index])
        .filter((id): id is number => typeof id === 'number');
      const es = await DatabaseService.deleteRows(
        selectedDfId,
        globalIndices,
        rowIds.length === indices.length ? rowIds : undefined,
      );
      await handleEditResult(es);
    } catch (e) { logger.data.error('deleteRows failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, rowOffset, loadedRowIds, handleEditResult]);

  const handleAddColumn = useCallback(async () => {
    if (!selectedDfId) return;
    const name = await uiStore.prompt({
      title: '新增列',
      label: '列名',
      placeholder: 'Column name',
    });
    if (!name) return;
    const dtype = await uiStore.prompt({
      title: '新增列',
      message: '请输入列类型：string, float64, int64, bool',
      label: '列类型',
      defaultValue: 'string',
      placeholder: 'string',
    });
    if (!dtype) return;
    try {
      const es = await DatabaseService.addColumn(selectedDfId, name, dtype);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('addColumn failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleDeleteColumn = useCallback(async (name: string) => {
    if (!selectedDfId) return;
    try {
      const es = await DatabaseService.deleteColumn(selectedDfId, name);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('deleteColumn failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleRenameColumn = useCallback(async (oldName: string) => {
    if (!selectedDfId) return;
    const newName = await uiStore.prompt({
      title: '重命名列',
      label: '新列名',
      defaultValue: oldName,
      placeholder: oldName,
    });
    if (!newName || newName === oldName) return;
    try {
      const es = await DatabaseService.renameColumn(selectedDfId, oldName, newName);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('renameColumn failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleCastColumn = useCallback(async (colName: string, newDtype: string) => {
    if (!selectedDfId) return;
    try {
      const es = await DatabaseService.castColumn(selectedDfId, colName, newDtype, false);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) {
      const msg = String(e);
      const force = await uiStore.confirm({
        title: '强制转换列类型',
        message: `${msg}\n\n是否强制转换？无法转换的值将变为 null。`,
        type: 'danger',
        confirmText: '强制转换',
      });
      if (force) {
        try {
          const es = await DatabaseService.castColumn(selectedDfId, colName, newDtype, true);
          await handleEditResult(es);
          const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
          useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
        } catch (e2) {
          const forceError = String(e2);
          logger.data.error('castColumn force failed: ' + forceError, 'DatabaseEditorWindow');
          uiStore.showToast(forceError, 'error', 5000);
        }
      }
    }
  }, [selectedDfId, handleEditResult]);

  return {
    currentEditState,
    commitCellValue,
    handleUndo,
    handleRedo,
    handleSave,
    handleExport,
    handleAddRow,
    handleDeleteRow,
    handleAddColumn,
    handleDeleteColumn,
    handleRenameColumn,
    handleCastColumn,
  };
}

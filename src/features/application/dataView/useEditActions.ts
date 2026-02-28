import { useState, useRef, useCallback } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore, useEditStateStore } from '@/features/core/dataStore';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { EMPTY_EDIT_STATE } from '@/features/core/dataStore/editStateStore';
import { logger } from '@/utils/appLogger';

export interface ColumnMeta { name: string; type: string; }

interface UseEditActionsParams {
  selectedDfId: string | null;
  columns: ColumnMeta[];
  loadedRows: any[][];
  reloadAllData: () => Promise<void>;
  loadColumnStats: (id: string) => Promise<void>;
}

export function useEditActions({ selectedDfId, columns, loadedRows, reloadAllData, loadColumnStats }: UseEditActionsParams) {
  const editStateByDatabase = useEditStateStore(s => s.editStateByDatabase);
  const editingCell = useEditStateStore(s => s.editingCell);
  const [editValue, setEditValue] = useState('');
  const editInputRef = useRef<HTMLInputElement>(null);

  const currentEditState: EditState = selectedDfId
    ? (editStateByDatabase[selectedDfId] ?? EMPTY_EDIT_STATE)
    : EMPTY_EDIT_STATE;

  const handleEditResult = useCallback(async (editState: EditState) => {
    if (!selectedDfId) return;
    useEditStateStore.getState().updateEditState(selectedDfId, editState);
    await reloadAllData();
  }, [selectedDfId, reloadAllData]);

  const startEdit = useCallback((row: number, col: number) => {
    const val = loadedRows[row]?.[col];
    setEditValue(val === null || val === undefined ? '' : String(val));
    useEditStateStore.getState().setEditingCell({ row, col });
    setTimeout(() => editInputRef.current?.focus(), 0);
  }, [loadedRows]);

  const commitEdit = useCallback(async () => {
    if (!selectedDfId || !editingCell) return;
    const { row, col } = editingCell;
    const colName = columns[col]?.name;
    if (!colName) return;
    const oldVal = loadedRows[row]?.[col];
    const oldStr = oldVal === null || oldVal === undefined ? '' : String(oldVal);
    if (editValue === oldStr) {
      useEditStateStore.getState().clearEditingCell();
      return;
    }
    try {
      let parsed: unknown = editValue;
      if (editValue === '') parsed = null;
      else if (!isNaN(Number(editValue)) && editValue.trim() !== '') parsed = Number(editValue);
      else parsed = editValue;
      const es = await DatabaseService.editCell(selectedDfId, row, colName, parsed);
      useEditStateStore.getState().clearEditingCell();
      await handleEditResult(es);
    } catch (e) {
      const msg = String(e);
      logger.data.error('editCell failed: ' + msg, 'DataViewWindow');
      window.alert(msg);
      editInputRef.current?.focus();
    }
  }, [selectedDfId, editingCell, columns, loadedRows, editValue, handleEditResult]);

  const cancelEdit = useCallback(() => {
    useEditStateStore.getState().clearEditingCell();
  }, []);

  const handleUndo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canUndo) return;
    try {
      const es = await DatabaseService.undoEdit(selectedDfId);
      await handleEditResult(es);
    } catch (e) { logger.data.error('undo failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, currentEditState.canUndo, handleEditResult]);

  const handleRedo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canRedo) return;
    try {
      const es = await DatabaseService.redoEdit(selectedDfId);
      await handleEditResult(es);
    } catch (e) { logger.data.error('redo failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, currentEditState.canRedo, handleEditResult]);

  const handleReset = useCallback(async () => {
    if (!selectedDfId || !currentEditState.isModified) return;
    try {
      const es = await DatabaseService.resetDatabase(selectedDfId);
      await handleEditResult(es);
      loadColumnStats(selectedDfId);
    } catch (e) { logger.data.error('reset failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, currentEditState.isModified, handleEditResult, loadColumnStats]);

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
    } catch (e) { logger.data.error('export failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId]);

  const handleAddRow = useCallback(async (index?: number) => {
    if (!selectedDfId) return;
    try {
      const es = await DatabaseService.addRow(selectedDfId, index);
      await handleEditResult(es);
    } catch (e) { logger.data.error('addRow failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleDeleteRow = useCallback(async (indices: number[]) => {
    if (!selectedDfId || indices.length === 0) return;
    try {
      const es = await DatabaseService.deleteRows(selectedDfId, indices);
      await handleEditResult(es);
    } catch (e) { logger.data.error('deleteRows failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleAddColumn = useCallback(async () => {
    if (!selectedDfId) return;
    const name = prompt('Column name:');
    if (!name) return;
    const dtype = prompt('Column type (string, float64, int64, bool):', 'string');
    if (!dtype) return;
    try {
      const es = await DatabaseService.addColumn(selectedDfId, name, dtype);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('addColumn failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleDeleteColumn = useCallback(async (name: string) => {
    if (!selectedDfId) return;
    try {
      const es = await DatabaseService.deleteColumn(selectedDfId, name);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('deleteColumn failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleRenameColumn = useCallback(async (oldName: string) => {
    if (!selectedDfId) return;
    const newName = prompt('New column name:', oldName);
    if (!newName || newName === oldName) return;
    try {
      const es = await DatabaseService.renameColumn(selectedDfId, oldName, newName);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
    } catch (e) { logger.data.error('renameColumn failed: ' + String(e), 'DataViewWindow'); }
  }, [selectedDfId, handleEditResult]);

  const handleCastColumn = useCallback(async (colName: string, newDtype: string) => {
    if (!selectedDfId) return;
    try {
      const es = await DatabaseService.castColumn(selectedDfId, colName, newDtype, false);
      await handleEditResult(es);
      const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
      useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
      loadColumnStats(selectedDfId);
    } catch (e) {
      const msg = String(e);
      const force = window.confirm(`${msg}\n\n是否强制转换？无法转换的值将变为 null。`);
      if (force) {
        try {
          const es = await DatabaseService.castColumn(selectedDfId, colName, newDtype, true);
          await handleEditResult(es);
          const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
          useDatabaseStore.getState().updateDatabase(selectedDfId, { columns: meta.columns, columnCount: meta.columnCount });
          loadColumnStats(selectedDfId);
        } catch (e2) { logger.data.error('castColumn force failed: ' + String(e2), 'DataViewWindow'); window.alert(String(e2)); }
      }
    }
  }, [selectedDfId, handleEditResult, loadColumnStats]);

  return {
    editingCell,
    editValue,
    setEditValue,
    editInputRef,
    currentEditState,
    startEdit,
    commitEdit,
    cancelEdit,
    handleUndo,
    handleRedo,
    handleReset,
    handleExport,
    handleAddRow,
    handleDeleteRow,
    handleAddColumn,
    handleDeleteColumn,
    handleRenameColumn,
    handleCastColumn,
  };
}

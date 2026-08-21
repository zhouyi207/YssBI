import { useCallback, useEffect, useRef } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { i18n } from '@/app/i18n';
import { DatabaseService } from '@/services/database/databaseService';
import { normalizeIpcError } from '@/services/ipc';
import { invalidateWorksheetPreviewCacheForDatabase } from '@/services/worksheet/worksheetPreviewCache';
import { useDatabaseStore, useEditStateStore } from '@/features/core/dataStore';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { EMPTY_EDIT_STATE } from '@/features/core/dataStore/editStateStore';
import { uiStore } from '@/features/core/ui/UIStore';
import type { ColumnInfo, DatabaseRow } from '@/shared/types/dto/database';
import { logger } from '@/utils/appLogger';
import { executeDatabaseMutation } from '@/features/application/dataManagement/databaseMutation';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  runDatabaseCellEditBatch,
  type DatabaseCellEditBatchOutcome,
  type DatabaseCellEditInput,
  type DatabaseCellEditStepOutcome,
} from './databaseCellEditBatch';

async function refreshDatabaseColumns(identity: ProjectIdentitySnapshot, databaseId: string) {
  const meta = await DatabaseService.getDatabaseMeta(identity.projectInstanceId, databaseId);
  if (!isCurrentProjectIdentity(identity)) return;
  useDatabaseStore.getState().updateDatabase(databaseId, {
    columns: meta.columns,
    columnCount: meta.columnCount,
  });
}

function showDatabaseEditorError(
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

export interface DatabaseEditorIpcFailure {
  code: string;
  incidentId: string | null;
}

export type DatabaseFieldMutationOutcome = DatabaseCellEditStepOutcome<DatabaseEditorIpcFailure>;
export type DatabaseCellBatchMutationOutcome = DatabaseCellEditBatchOutcome<DatabaseEditorIpcFailure>;

interface UseEditActionsParams {
  selectedDfId: string | null;
  columns: ColumnInfo[];
  loadedRows: DatabaseRow[];
  loadedRowIds: number[];
  rowOffset: number;
  reloadAllData: () => Promise<void>;
  getDataScopeVersion: () => number;
}

export function useEditActions({
  selectedDfId,
  columns,
  loadedRows,
  loadedRowIds,
  rowOffset,
  reloadAllData,
  getDataScopeVersion,
}: UseEditActionsParams) {
  const editStateByDatabase = useEditStateStore(s => s.editStateByDatabase);
  const cellEditQueueRef = useRef<Promise<void>>(Promise.resolve());
  const cellEditScopeRef = useRef({ databaseId: selectedDfId, rowOffset, version: 0 });
  if (cellEditScopeRef.current.databaseId !== selectedDfId
    || cellEditScopeRef.current.rowOffset !== rowOffset) {
    cellEditScopeRef.current = {
      databaseId: selectedDfId,
      rowOffset,
      version: cellEditScopeRef.current.version + 1,
    };
  }

  useEffect(() => () => {
    cellEditScopeRef.current = {
      ...cellEditScopeRef.current,
      version: cellEditScopeRef.current.version + 1,
    };
  }, []);

  const currentEditState: EditState = selectedDfId
    ? (editStateByDatabase[selectedDfId] ?? EMPTY_EDIT_STATE)
    : EMPTY_EDIT_STATE;

  const handleEditResult = useCallback(async (editState: EditState) => {
    if (!selectedDfId) return;
    invalidateWorksheetPreviewCacheForDatabase(selectedDfId);
    useEditStateStore.getState().updateEditState(selectedDfId, editState);
    await reloadAllData();
  }, [selectedDfId, reloadAllData]);

  const enqueueCellEdit = useCallback(<T,>(operation: () => Promise<T>): Promise<T> => {
    const result = cellEditQueueRef.current.then(operation, operation);
    cellEditQueueRef.current = result.then(() => undefined, () => undefined);
    return result;
  }, []);

  const applyCellEdit = useCallback(async (
    edit: DatabaseCellEditInput,
    identity: ProjectIdentitySnapshot,
  ): Promise<DatabaseFieldMutationOutcome> => {
    if (!selectedDfId) return { status: 'noop' };
    const colName = columns[edit.column]?.name;
    if (!colName || !loadedRows[edit.row]) return { status: 'noop' };

    const nextText = edit.value === null || edit.value === undefined ? '' : String(edit.value);

    try {
      let parsed: unknown = edit.value;
      if (nextText === '') parsed = null;
      else if (typeof edit.value === 'string'
        && !isNaN(Number(edit.value))
        && edit.value.trim() !== '') {
        parsed = Number(edit.value);
      }
      const editState = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.editCell(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          rowOffset + edit.row,
          colName,
          parsed,
          loadedRowIds[edit.row],
        ));
      if (isCurrentProjectIdentity(identity)) {
        invalidateWorksheetPreviewCacheForDatabase(selectedDfId);
        useEditStateStore.getState().updateEditState(selectedDfId, editState);
      }
      return { status: 'applied' };
    } catch (error) {
      if (!isCurrentProjectIdentity(identity)) return { status: 'noop' };
      const ipcError = normalizeIpcError('edit_database_cell', error);
      logger.data.error(
        `editCell failed code=${ipcError.code} incidentId=${ipcError.incidentId ?? 'none'}`,
        'DatabaseEditorWindow',
      );
      return {
        status: 'failed',
        error: { code: ipcError.code, incidentId: ipcError.incidentId },
      };
    }
  }, [selectedDfId, columns, loadedRows, loadedRowIds, rowOffset]);

  const commitCellValuesOutcome = useCallback((
    edits: readonly DatabaseCellEditInput[],
  ): Promise<DatabaseCellBatchMutationOutcome> => {
    if (!selectedDfId) return Promise.resolve({ status: 'noop', appliedCount: 0 });
    const identity = captureProjectIdentity();
    const scopeVersion = cellEditScopeRef.current.version;
    const dataScopeVersion = getDataScopeVersion();
    return enqueueCellEdit(() => runDatabaseCellEditBatch({
      edits,
      apply: (edit) => applyCellEdit(edit, identity),
      isCurrent: () => isCurrentProjectIdentity(identity)
        && cellEditScopeRef.current.version === scopeVersion
        && getDataScopeVersion() === dataScopeVersion,
      refresh: reloadAllData,
    }));
  }, [
    selectedDfId,
    applyCellEdit,
    enqueueCellEdit,
    getDataScopeVersion,
    reloadAllData,
  ]);

  const commitCellValues = useCallback(async (
    edits: readonly DatabaseCellEditInput[],
  ): Promise<DatabaseCellBatchMutationOutcome> => {
    const outcome = await commitCellValuesOutcome(edits);
    if (outcome.status === 'failed') {
      void uiStore.alert({
        title: i18n.t('common.error'),
        message: i18n.t('notifications.databaseEditor.pasteFailed', {
          appliedCount: outcome.appliedCount,
          error: outcome.error.code,
        }),
        closeText: i18n.t('common.close'),
        type: 'error',
        incidentId: outcome.error.incidentId,
        incidentLabel: i18n.t('common.incidentId'),
      });
    }
    return outcome;
  }, [commitCellValuesOutcome]);

  const commitCellValueOutcome = useCallback(async (
    row: number,
    col: number,
    value: unknown,
  ): Promise<DatabaseFieldMutationOutcome> => {
    const outcome = await commitCellValuesOutcome([{ row, column: col, value }]);
    if (outcome.status === 'failed') return { status: 'failed', error: outcome.error };
    return outcome.status === 'applied' ? { status: 'applied' } : { status: 'noop' };
  }, [commitCellValuesOutcome]);

  const commitCellValue = useCallback(async (row: number, col: number, value: unknown) => {
    await commitCellValueOutcome(row, col, value);
  }, [commitCellValueOutcome]);

  const handleUndo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canUndo) return;
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.undoEdit(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
        ));
      await handleEditResult(es);
    } catch (e) { logger.data.error('undo failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, currentEditState.canUndo, handleEditResult]);

  const handleRedo = useCallback(async () => {
    if (!selectedDfId || !currentEditState.canRedo) return;
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.redoEdit(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
        ));
      await handleEditResult(es);
    } catch (e) { logger.data.error('redo failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, currentEditState.canRedo, handleEditResult]);

  const handleSave = useCallback(async () => {
    if (!selectedDfId || !currentEditState.isModified) return;
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.saveDatabaseChanges(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
        ));
      await handleEditResult(es);
    } catch (e) {
      logger.data.error('save changes failed: ' + String(e), 'DatabaseEditorWindow');
      showDatabaseEditorError(e, 'save_database_changes', (code) =>
        i18n.t('notifications.databaseEditor.saveFailed', { error: code }));
    }
  }, [selectedDfId, currentEditState.isModified, handleEditResult]);

  const handleExport = useCallback(async () => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    try {
      const filePath = await save({
        title: 'Export Data',
        filters: [
          { name: 'CSV', extensions: ['csv'] },
          { name: 'Parquet', extensions: ['parquet'] },
        ],
      });
      if (!isCurrentProjectIdentity(identity) || !filePath) return;
      const fmt = filePath.endsWith('.parquet') ? 'parquet' : 'csv';
      await DatabaseService.exportDatabase(
        identity.projectInstanceId,
        selectedDfId,
        filePath,
        fmt,
      );
      if (!isCurrentProjectIdentity(identity)) return;
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('export failed: ' + String(e), 'DatabaseEditorWindow');
      }
    }
  }, [selectedDfId]);

  const handleAddRow = useCallback(async (index?: number) => {
    if (!selectedDfId) return;
    try {
      const globalIndex = index === undefined ? undefined : rowOffset + index;
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.addRow(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          globalIndex,
        ));
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
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.deleteRows(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          globalIndices,
          rowIds.length === indices.length ? rowIds : undefined,
        ));
      await handleEditResult(es);
    } catch (e) { logger.data.error('deleteRows failed: ' + String(e), 'DatabaseEditorWindow'); }
  }, [selectedDfId, rowOffset, loadedRowIds, handleEditResult]);

  const handleAddColumn = useCallback(async () => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    const name = await uiStore.prompt({
      title: '新增列',
      label: '列名',
      placeholder: 'Column name',
    });
    if (!isCurrentProjectIdentity(identity) || !name) return;
    const dtype = await uiStore.prompt({
      title: '新增列',
      message: '请输入列类型：string, float64, int64, bool',
      label: '列类型',
      defaultValue: 'string',
      placeholder: 'string',
    });
    if (!isCurrentProjectIdentity(identity) || !dtype) return;
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.addColumn(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          name,
          dtype,
        ));
      if (!isCurrentProjectIdentity(identity)) return;
      await handleEditResult(es);
      if (!isCurrentProjectIdentity(identity)) return;
      await refreshDatabaseColumns(identity, selectedDfId);
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('addColumn failed: ' + String(e), 'DatabaseEditorWindow');
      }
    }
  }, [selectedDfId, handleEditResult]);

  const handleDeleteColumn = useCallback(async (name: string) => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.deleteColumn(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          name,
        ));
      if (!isCurrentProjectIdentity(identity)) return;
      await handleEditResult(es);
      if (!isCurrentProjectIdentity(identity)) return;
      await refreshDatabaseColumns(identity, selectedDfId);
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('deleteColumn failed: ' + String(e), 'DatabaseEditorWindow');
      }
    }
  }, [selectedDfId, handleEditResult]);

  const handleRenameColumn = useCallback(async (oldName: string) => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    const newName = await uiStore.prompt({
      title: '重命名列',
      label: '新列名',
      defaultValue: oldName,
      placeholder: oldName,
    });
    if (!isCurrentProjectIdentity(identity) || !newName || newName === oldName) return;
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.renameColumn(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          oldName,
          newName,
        ));
      if (!isCurrentProjectIdentity(identity)) return;
      await handleEditResult(es);
      if (!isCurrentProjectIdentity(identity)) return;
      await refreshDatabaseColumns(identity, selectedDfId);
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('renameColumn failed: ' + String(e), 'DatabaseEditorWindow');
      }
    }
  }, [selectedDfId, handleEditResult]);

  const handleCastColumn = useCallback(async (colName: string, newDtype: string) => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    try {
      const es = await executeDatabaseMutation(selectedDfId, (authority) =>
        DatabaseService.castColumn(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          selectedDfId,
          colName,
          newDtype,
          false,
        ));
      if (!isCurrentProjectIdentity(identity)) return;
      await handleEditResult(es);
      if (!isCurrentProjectIdentity(identity)) return;
      await refreshDatabaseColumns(identity, selectedDfId);
    } catch (e) {
      if (!isCurrentProjectIdentity(identity)) return;
      const ipcError = normalizeIpcError('cast_database_column', e);
      const reference = ipcError.incidentId
        ? `${ipcError.code}\n${i18n.t('common.incidentId')}: ${ipcError.incidentId}`
        : ipcError.code;
      const force = await uiStore.confirm({
        title: '强制转换列类型',
        message: `${reference}\n\n是否强制转换？无法转换的值将变为 null。`,
        type: 'danger',
        confirmText: '强制转换',
      });
      if (force && isCurrentProjectIdentity(identity)) {
        try {
          const es = await executeDatabaseMutation(selectedDfId, (authority) =>
            DatabaseService.castColumn(
              authority.projectInstanceId,
              authority.operationId,
              authority.expectedRevision,
              selectedDfId,
              colName,
              newDtype,
              true,
            ));
          if (!isCurrentProjectIdentity(identity)) return;
          await handleEditResult(es);
          if (!isCurrentProjectIdentity(identity)) return;
          await refreshDatabaseColumns(identity, selectedDfId);
        } catch (e2) {
          if (isCurrentProjectIdentity(identity)) {
            logger.data.error('castColumn force failed: ' + String(e2), 'DatabaseEditorWindow');
            showDatabaseEditorError(e2, 'cast_database_column', (code) =>
              i18n.t('notifications.databaseEditor.forceCastFailed', { error: code }));
          }
        }
      }
    }
  }, [selectedDfId, handleEditResult]);

  return {
    currentEditState,
    commitCellValue,
    commitCellValueOutcome,
    commitCellValues,
    commitCellValuesOutcome,
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

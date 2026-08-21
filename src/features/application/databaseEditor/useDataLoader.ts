import { useState, useRef, useCallback } from 'react';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { DATABASE_EDITOR_CHUNK_SIZE } from '@/app/appConfig/default';
import type { DatabaseRow } from '@/shared/types/dto/database';
import { logger } from '@/utils/appLogger';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

export function useDataLoader(selectedDfId: string | null) {
  const selectedRowCount = useDatabaseStore(s => selectedDfId ? (s.databases[selectedDfId]?.rowCount ?? 0) : 0);
  const [loadedRows, setLoadedRows] = useState<DatabaseRow[]>([]);
  const [loadedRowIds, setLoadedRowIds] = useState<number[]>([]);
  const [loading, setLoading] = useState(true);
  const [pageIndex, setPageIndex] = useState(0);
  const [lastFetchMs, setLastFetchMs] = useState<number | null>(null);
  const rowsRequestEpochRef = useRef(0);
  const pageIntentEpochRef = useRef(0);
  const selectedDfIdRef = useRef(selectedDfId);
  selectedDfIdRef.current = selectedDfId;

  const CHUNK_SIZE = DATABASE_EDITOR_CHUNK_SIZE;
  const totalPages = Math.max(1, Math.ceil(selectedRowCount / CHUNK_SIZE));
  const pageStartIndex = pageIndex * CHUNK_SIZE;

  const loadPageRowsForIdentity = useCallback(async (
    identity: ProjectIdentitySnapshot,
    id: string,
    nextPageIndex: number,
  ) => {
    const requestEpoch = ++rowsRequestEpochRef.current;
    const rowCount = useDatabaseStore.getState().databases[id]?.rowCount ?? 0;
    const maxPageIndex = rowCount > 0
      ? Math.max(0, Math.ceil(rowCount / CHUNK_SIZE) - 1)
      : nextPageIndex;
    const safePageIndex = Math.max(0, Math.min(nextPageIndex, maxPageIndex));
    setLoading(true);
    const startedAt = performance.now();
    try {
      const page = await DatabaseService.getDatabaseRows(
        identity.projectInstanceId,
        id,
        safePageIndex * CHUNK_SIZE,
        CHUNK_SIZE,
      );
      if (!isCurrentProjectIdentity(identity)
        || requestEpoch !== rowsRequestEpochRef.current
        || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(page.rows);
      setLoadedRowIds(page.rowIds);
      setLastFetchMs(Math.round(performance.now() - startedAt));
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('Failed to load page rows: ' + String(e), 'DatabaseEditorWindow');
      }
    } finally {
      if (isCurrentProjectIdentity(identity) && requestEpoch === rowsRequestEpochRef.current) {
        setLoading(false);
      }
    }
  }, [CHUNK_SIZE]);

  const loadPageRows = useCallback(async (id: string, nextPageIndex: number) => {
    const identity = captureProjectIdentity();
    await loadPageRowsForIdentity(identity, id, nextPageIndex);
  }, [loadPageRowsForIdentity]);

  const ensureDatabaseMeta = useCallback(async (identity: ProjectIdentitySnapshot, id: string) => {
    const db = useDatabaseStore.getState().databases[id];
    const hasColumns = (db?.columns?.length ?? 0) > 0;
    const hasRowCount = (db?.rowCount ?? 0) > 0;
    if (hasColumns && hasRowCount) return;

    const meta = await DatabaseService.getDatabaseMeta(identity.projectInstanceId, id);
    if (!isCurrentProjectIdentity(identity)) return;
    useDatabaseStore.getState().updateDatabase(id, {
      name: meta.name,
      columns: meta.columns,
      rowCount: meta.rowCount,
      columnCount: meta.columnCount,
    });
  }, []);

  const loadInitialRows = useCallback(async (id: string) => {
    const identity = captureProjectIdentity();
    pageIntentEpochRef.current += 1;
    rowsRequestEpochRef.current += 1;
    try {
      await ensureDatabaseMeta(identity, id);
      if (!isCurrentProjectIdentity(identity)) return;
    } catch (e) {
      if (!isCurrentProjectIdentity(identity)) return;
      logger.data.warn('getDatabaseMeta failed before row load: ' + String(e), 'DatabaseEditorWindow');
    }
    await loadPageRowsForIdentity(identity, id, 0);
  }, [ensureDatabaseMeta, loadPageRowsForIdentity]);

  const reloadAllData = useCallback(async () => {
    if (!selectedDfId) return;
    const identity = captureProjectIdentity();
    const requestEpoch = ++rowsRequestEpochRef.current;
    const id = selectedDfId;
    const safePageIndex = Math.max(0, Math.min(pageIndex, Math.max(0, totalPages - 1)));
    const startedAt = performance.now();
    try {
      const page = await DatabaseService.getDatabaseRows(
        identity.projectInstanceId,
        id,
        safePageIndex * CHUNK_SIZE,
        CHUNK_SIZE,
      );
      if (!isCurrentProjectIdentity(identity)
        || requestEpoch !== rowsRequestEpochRef.current
        || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(page.rows);
      setLoadedRowIds(page.rowIds);
      const meta = await DatabaseService.getDatabaseMeta(identity.projectInstanceId, id);
      if (!isCurrentProjectIdentity(identity)
        || requestEpoch !== rowsRequestEpochRef.current
        || id !== selectedDfIdRef.current) return;
      useDatabaseStore.getState().updateDatabase(id, {
        name: meta.name,
        columns: meta.columns,
        rowCount: meta.rowCount,
        columnCount: meta.columnCount,
      });
      setLastFetchMs(Math.round(performance.now() - startedAt));
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('Failed to reload data: ' + String(e), 'DatabaseEditorWindow');
      }
    }
  }, [selectedDfId, pageIndex, totalPages, CHUNK_SIZE]);

  const goToPage = useCallback(async (nextPageIndex: number) => {
    if (!selectedDfId) return;
    pageIntentEpochRef.current += 1;
    await loadPageRows(selectedDfId, nextPageIndex);
  }, [selectedDfId, loadPageRows]);

  const goToPreviousPage = useCallback(async () => {
    await goToPage(pageIndex - 1);
  }, [goToPage, pageIndex]);

  const goToNextPage = useCallback(async () => {
    await goToPage(pageIndex + 1);
  }, [goToPage, pageIndex]);

  const refreshData = useCallback(async () => {
    const identity = captureProjectIdentity();
    pageIntentEpochRef.current += 1;
    const requestEpoch = ++rowsRequestEpochRef.current;
    setLoading(true);
    const startedAt = performance.now();
    try {
      await initProjectSync();
      if (!isCurrentProjectIdentity(identity)
        || requestEpoch !== rowsRequestEpochRef.current) return;
      if (selectedDfId) {
        const id = selectedDfId;
        const safePageIndex = Math.max(0, Math.min(pageIndex, Math.max(0, totalPages - 1)));
        const page = await DatabaseService.getDatabaseRows(
          identity.projectInstanceId,
          id,
          safePageIndex * CHUNK_SIZE,
          CHUNK_SIZE,
        );
        if (!isCurrentProjectIdentity(identity)
          || requestEpoch !== rowsRequestEpochRef.current
          || id !== selectedDfIdRef.current) return;
        setPageIndex(safePageIndex);
        setLoadedRows(page.rows);
        setLoadedRowIds(page.rowIds);
        setLastFetchMs(Math.round(performance.now() - startedAt));
      }
    } catch (e) {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('Failed to fetch dataframes: ' + String(e), 'DatabaseEditorWindow');
      }
    } finally {
      if (isCurrentProjectIdentity(identity) && requestEpoch === rowsRequestEpochRef.current) {
        setLoading(false);
      }
    }
  }, [selectedDfId, pageIndex, totalPages, CHUNK_SIZE]);

  const getPageIntentEpoch = useCallback(() => pageIntentEpochRef.current, []);

  return {
    loadedRows,
    setLoadedRows,
    loadedRowIds,
    setLoadedRowIds,
    loading,
    CHUNK_SIZE,
    pageIndex,
    pageSize: CHUNK_SIZE,
    pageStartIndex,
    lastFetchMs,
    totalPages,
    loadInitialRows,
    reloadAllData,
    goToPage,
    goToPreviousPage,
    goToNextPage,
    refreshData,
    getPageIntentEpoch,
  };
}

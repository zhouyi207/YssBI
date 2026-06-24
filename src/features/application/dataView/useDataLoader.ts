import { useState, useRef, useCallback } from 'react';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';
import { logger } from '@/utils/appLogger';

export function useDataLoader(selectedDfId: string | null) {
  const selectedRowCount = useDatabaseStore(s => selectedDfId ? ((s.databases[selectedDfId]?.rowCount as number) ?? 0) : 0);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
  const [loadedRowIds, setLoadedRowIds] = useState<number[]>([]);
  const [loading, setLoading] = useState(true);
  const [pageIndex, setPageIndex] = useState(0);
  const [lastFetchMs, setLastFetchMs] = useState<number | null>(null);
  const initialRowsRequestRef = useRef(0);
  const reloadRequestRef = useRef(0);
  const refreshRequestRef = useRef(0);
  const selectedDfIdRef = useRef(selectedDfId);
  selectedDfIdRef.current = selectedDfId;

  const CHUNK_SIZE = DATA_VIEW_CHUNK_SIZE;
  const totalPages = Math.max(1, Math.ceil(selectedRowCount / CHUNK_SIZE));
  const pageStartIndex = pageIndex * CHUNK_SIZE;

  const loadPageRows = useCallback(async (id: string, nextPageIndex: number) => {
    const requestId = ++initialRowsRequestRef.current;
    const rowCount = (useDatabaseStore.getState().databases[id]?.rowCount as number | undefined) ?? 0;
    const maxPageIndex = rowCount > 0
      ? Math.max(0, Math.ceil(rowCount / CHUNK_SIZE) - 1)
      : nextPageIndex;
    const safePageIndex = Math.max(0, Math.min(nextPageIndex, maxPageIndex));
    setLoading(true);
    const startedAt = performance.now();
    try {
      const page = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
      if (requestId !== initialRowsRequestRef.current || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(page.rows);
      setLoadedRowIds(page.rowIds);
      setLastFetchMs(Math.round(performance.now() - startedAt));
    } catch (e) {
      logger.data.error('Failed to load page rows: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === initialRowsRequestRef.current) setLoading(false);
    }
  }, [CHUNK_SIZE]);

  const ensureDatabaseMeta = useCallback(async (id: string) => {
    const db = useDatabaseStore.getState().databases[id] as Record<string, unknown> | undefined;
    const hasColumns = Array.isArray(db?.columns) && (db.columns as unknown[]).length > 0;
    const hasRowCount = typeof db?.rowCount === 'number' && (db.rowCount as number) > 0;
    if (hasColumns && hasRowCount) return;

    const meta = await DatabaseService.getDatabaseMeta(id);
    useDatabaseStore.getState().updateDatabase(id, {
      name: meta.name,
      columns: meta.columns,
      rowCount: meta.rowCount,
      columnCount: meta.columnCount,
    });
  }, []);

  const loadInitialRows = useCallback(async (id: string) => {
    try {
      await ensureDatabaseMeta(id);
    } catch (e) {
      logger.data.warn('getDatabaseMeta failed before row load: ' + String(e), 'DataViewWindow');
    }
    await loadPageRows(id, 0);
  }, [ensureDatabaseMeta, loadPageRows]);

  const reloadAllData = useCallback(async () => {
    if (!selectedDfId) return;
    const requestId = ++reloadRequestRef.current;
    const id = selectedDfId;
    const safePageIndex = Math.max(0, Math.min(pageIndex, Math.max(0, totalPages - 1)));
    const startedAt = performance.now();
    try {
      const page = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
      if (requestId !== reloadRequestRef.current || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(page.rows);
      setLoadedRowIds(page.rowIds);
      const meta = await DatabaseService.getDatabaseMeta(id);
      if (requestId !== reloadRequestRef.current || id !== selectedDfIdRef.current) return;
      useDatabaseStore.getState().updateDatabase(id, {
        name: meta.name,
        columns: meta.columns,
        rowCount: meta.rowCount,
        columnCount: meta.columnCount,
      });
      setLastFetchMs(Math.round(performance.now() - startedAt));
    } catch (e) {
      logger.data.error('Failed to reload data: ' + String(e), 'DataViewWindow');
    }
  }, [selectedDfId, pageIndex, totalPages, CHUNK_SIZE]);

  const goToPage = useCallback(async (nextPageIndex: number) => {
    if (!selectedDfId) return;
    await loadPageRows(selectedDfId, nextPageIndex);
  }, [selectedDfId, loadPageRows]);

  const goToPreviousPage = useCallback(async () => {
    await goToPage(pageIndex - 1);
  }, [goToPage, pageIndex]);

  const goToNextPage = useCallback(async () => {
    await goToPage(pageIndex + 1);
  }, [goToPage, pageIndex]);

  const refreshData = useCallback(async () => {
    const requestId = ++refreshRequestRef.current;
    setLoading(true);
    const startedAt = performance.now();
    try {
      await initProjectSync();
      if (selectedDfId) {
        const id = selectedDfId;
        const safePageIndex = Math.max(0, Math.min(pageIndex, Math.max(0, totalPages - 1)));
        const page = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
        if (requestId !== refreshRequestRef.current || id !== selectedDfIdRef.current) return;
        setPageIndex(safePageIndex);
        setLoadedRows(page.rows);
        setLoadedRowIds(page.rowIds);
        setLastFetchMs(Math.round(performance.now() - startedAt));
      }
    } catch (e) {
      logger.data.error('Failed to fetch dataframes: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === refreshRequestRef.current) setLoading(false);
    }
  }, [selectedDfId, pageIndex, totalPages, CHUNK_SIZE]);

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
  };
}

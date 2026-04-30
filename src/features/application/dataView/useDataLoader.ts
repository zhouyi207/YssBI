import { useState, useRef, useCallback } from 'react';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';
import { logger } from '@/utils/appLogger';

export function useDataLoader(selectedDfId: string | null) {
  const selectedRowCount = useDatabaseStore(s => selectedDfId ? ((s.databases[selectedDfId]?.rowCount as number) ?? 0) : 0);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
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
    const safePageIndex = Math.max(0, Math.min(nextPageIndex, Math.max(0, Math.ceil(selectedRowCount / CHUNK_SIZE) - 1)));
    setLoading(true);
    const startedAt = performance.now();
    try {
      const rows = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
      if (requestId !== initialRowsRequestRef.current || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(rows);
      setLastFetchMs(Math.round(performance.now() - startedAt));
    } catch (e) {
      logger.data.error('Failed to load page rows: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === initialRowsRequestRef.current) setLoading(false);
    }
  }, [CHUNK_SIZE, selectedRowCount]);

  const loadInitialRows = useCallback(async (id: string) => {
    await loadPageRows(id, 0);
  }, [loadPageRows]);

  const reloadAllData = useCallback(async () => {
    if (!selectedDfId) return;
    const requestId = ++reloadRequestRef.current;
    const id = selectedDfId;
    const safePageIndex = Math.max(0, Math.min(pageIndex, Math.max(0, totalPages - 1)));
    const startedAt = performance.now();
    try {
      const rows = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
      if (requestId !== reloadRequestRef.current || id !== selectedDfIdRef.current) return;
      setPageIndex(safePageIndex);
      setLoadedRows(rows);
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
        const rows = await DatabaseService.getDatabaseRows(id, safePageIndex * CHUNK_SIZE, CHUNK_SIZE);
        if (requestId !== refreshRequestRef.current || id !== selectedDfIdRef.current) return;
        setPageIndex(safePageIndex);
        setLoadedRows(rows);
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

import { useState, useRef, useCallback } from 'react';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseStore, useColumnStatsStore, useColumnDistributionStore, useDatasetOverviewStore, initProjectSync } from '@/features/core/dataStore';
import { DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';
import { logger } from '@/utils/appLogger';

export function useDataLoader(selectedDfId: string | null) {
  const dataframes = useDatabaseStore(s => s.databases);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [statsLoading, setStatsLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastScrollTop = useRef<number>(0);
  const initialRowsRequestRef = useRef(0);
  const statsRequestRef = useRef(0);
  const reloadRequestRef = useRef(0);
  const loadMoreRequestRef = useRef(0);
  const refreshRequestRef = useRef(0);
  const selectedDfIdRef = useRef(selectedDfId);
  selectedDfIdRef.current = selectedDfId;

  const CHUNK_SIZE = DATA_VIEW_CHUNK_SIZE;

  const loadInitialRows = useCallback(async (id: string) => {
    const requestId = ++initialRowsRequestRef.current;
    setLoading(true);
    try {
      const rows = await DatabaseService.getDatabaseRows(id, 0, CHUNK_SIZE);
      if (requestId !== initialRowsRequestRef.current || id !== selectedDfIdRef.current) return;
      setLoadedRows(rows);
    } catch (e) {
      logger.data.error('Failed to load initial rows: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === initialRowsRequestRef.current) setLoading(false);
    }
  }, [CHUNK_SIZE]);

  const loadColumnStats = useCallback(async (id: string) => {
    const requestId = ++statsRequestRef.current;
    setStatsLoading(true);
    try {
      const [stats, dists, overview] = await Promise.all([
        DatabaseService.getColumnStats(id),
        DatabaseService.getColumnDistribution(id),
        DatabaseService.getDatasetOverview(id),
      ]);
      if (requestId !== statsRequestRef.current || id !== selectedDfIdRef.current) return;
      useColumnStatsStore.getState().setAllStats(id, stats);
      useColumnDistributionStore.getState().setAllDistributions(id, dists);
      useDatasetOverviewStore.getState().setOverview(id, overview);
    } catch (e) {
      logger.data.error('Failed to load column stats: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === statsRequestRef.current) setStatsLoading(false);
    }
  }, []);

  const reloadAllData = useCallback(async () => {
    if (!selectedDfId) return;
    const requestId = ++reloadRequestRef.current;
    const id = selectedDfId;
    try {
      const rows = await DatabaseService.getDatabaseRows(id, 0, Math.max(loadedRows.length, CHUNK_SIZE));
      if (requestId !== reloadRequestRef.current || id !== selectedDfIdRef.current) return;
      setLoadedRows(rows);
      const meta = await DatabaseService.getDatabaseMeta(id);
      if (requestId !== reloadRequestRef.current || id !== selectedDfIdRef.current) return;
      useDatabaseStore.getState().updateDatabase(id, {
        name: meta.name,
        columns: meta.columns,
        rowCount: meta.rowCount,
        columnCount: meta.columnCount,
      });
      await loadColumnStats(id);
    } catch (e) {
      logger.data.error('Failed to reload data: ' + String(e), 'DataViewWindow');
    }
  }, [selectedDfId, loadedRows.length, CHUNK_SIZE, loadColumnStats]);

  const loadMoreRows = useCallback(async () => {
    if (!selectedDfId || loadingMore) return;
    const currentCount = loadedRows.length;
    const totalCount = (dataframes[selectedDfId]?.rowCount as number) ?? 0;
    if (currentCount >= totalCount) return;
    const requestId = ++loadMoreRequestRef.current;
    const id = selectedDfId;
    setLoadingMore(true);
    try {
      const newRows = await DatabaseService.getDatabaseRows(id, currentCount, CHUNK_SIZE);
      if (requestId !== loadMoreRequestRef.current || id !== selectedDfIdRef.current) return;
      setLoadedRows(prev => [...prev, ...newRows]);
    } catch (e) {
      logger.data.error('Failed to load more rows: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === loadMoreRequestRef.current) setLoadingMore(false);
    }
  }, [selectedDfId, loadingMore, loadedRows.length, dataframes, CHUNK_SIZE]);

  const refreshData = useCallback(async () => {
    if (scrollRef.current) {
      lastScrollTop.current = scrollRef.current.scrollTop;
    }
    const requestId = ++refreshRequestRef.current;
    setLoading(true);
    try {
      await initProjectSync();
      if (selectedDfId) {
        const id = selectedDfId;
        const rows = await DatabaseService.getDatabaseRows(id, 0, Math.max(loadedRows.length, CHUNK_SIZE));
        if (requestId !== refreshRequestRef.current || id !== selectedDfIdRef.current) return;
        setLoadedRows(rows);
        await loadColumnStats(id);
        setTimeout(() => {
          if (scrollRef.current) scrollRef.current.scrollTop = lastScrollTop.current;
        }, 0);
      }
    } catch (e) {
      logger.data.error('Failed to fetch dataframes: ' + String(e), 'DataViewWindow');
    } finally {
      if (requestId === refreshRequestRef.current) setLoading(false);
    }
  }, [selectedDfId, loadedRows.length, CHUNK_SIZE, loadColumnStats]);

  return {
    loadedRows,
    setLoadedRows,
    loading,
    loadingMore,
    statsLoading,
    scrollRef,
    CHUNK_SIZE,
    loadInitialRows,
    loadColumnStats,
    reloadAllData,
    loadMoreRows,
    refreshData,
  };
}

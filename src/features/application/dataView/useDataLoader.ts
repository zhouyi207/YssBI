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

  const CHUNK_SIZE = DATA_VIEW_CHUNK_SIZE;

  const loadInitialRows = useCallback(async (id: string) => {
    setLoading(true);
    try {
      const rows = await DatabaseService.getDatabaseRows(id, 0, CHUNK_SIZE);
      setLoadedRows(rows);
    } catch (e) {
      logger.data.error('Failed to load initial rows: ' + String(e), 'DataViewWindow');
    } finally {
      setLoading(false);
    }
  }, [CHUNK_SIZE]);

  const loadColumnStats = useCallback(async (id: string) => {
    setStatsLoading(true);
    try {
      const [stats, dists, overview] = await Promise.all([
        DatabaseService.getColumnStats(id),
        DatabaseService.getColumnDistribution(id),
        DatabaseService.getDatasetOverview(id),
      ]);
      useColumnStatsStore.getState().setAllStats(id, stats);
      useColumnDistributionStore.getState().setAllDistributions(id, dists);
      useDatasetOverviewStore.getState().setOverview(id, overview);
    } catch (e) {
      logger.data.error('Failed to load column stats: ' + String(e), 'DataViewWindow');
    } finally {
      setStatsLoading(false);
    }
  }, []);

  const reloadAllData = useCallback(async () => {
    if (!selectedDfId) return;
    const rows = await DatabaseService.getDatabaseRows(selectedDfId, 0, Math.max(loadedRows.length, CHUNK_SIZE));
    setLoadedRows(rows);
    const meta = await DatabaseService.getDatabaseMeta(selectedDfId);
    useDatabaseStore.getState().updateDatabase(selectedDfId, {
      name: meta.name,
      columns: meta.columns,
      rowCount: meta.rowCount,
      columnCount: meta.columnCount,
    });
  }, [selectedDfId, loadedRows.length, CHUNK_SIZE]);

  const loadMoreRows = useCallback(async () => {
    if (!selectedDfId || loadingMore) return;
    const currentCount = loadedRows.length;
    const totalCount = (dataframes[selectedDfId]?.rowCount as number) ?? 0;
    if (currentCount >= totalCount) return;
    setLoadingMore(true);
    try {
      const newRows = await DatabaseService.getDatabaseRows(selectedDfId, currentCount, CHUNK_SIZE);
      setLoadedRows(prev => [...prev, ...newRows]);
    } catch (e) {
      logger.data.error('Failed to load more rows: ' + String(e), 'DataViewWindow');
    } finally {
      setLoadingMore(false);
    }
  }, [selectedDfId, loadingMore, loadedRows.length, dataframes, CHUNK_SIZE]);

  const refreshData = useCallback(async () => {
    if (scrollRef.current) {
      lastScrollTop.current = scrollRef.current.scrollTop;
    }
    setLoading(true);
    try {
      await initProjectSync();
      if (selectedDfId) {
        const rows = await DatabaseService.getDatabaseRows(selectedDfId, 0, Math.max(loadedRows.length, CHUNK_SIZE));
        setLoadedRows(rows);
        loadColumnStats(selectedDfId);
        setTimeout(() => {
          if (scrollRef.current) scrollRef.current.scrollTop = lastScrollTop.current;
        }, 0);
      }
    } catch (e) {
      logger.data.error('Failed to fetch dataframes: ' + String(e), 'DataViewWindow');
    } finally {
      setLoading(false);
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

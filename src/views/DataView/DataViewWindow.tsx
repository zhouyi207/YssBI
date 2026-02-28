import React, { useEffect, useState, useRef, useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DatabaseService } from '@/services/database/databaseService';
import { VscDatabase, VscRefresh, VscSymbolNumeric } from 'react-icons/vsc';
import { useProjectSync } from '@/features/application/initialization';
import { useDatabaseStore, useColumnStatsStore, useColumnDistributionStore, useDatasetOverviewStore, initProjectSync } from '@/features/core/dataStore';
import type { ColumnStats, NumericColumnStats, StringColumnStats } from '@/features/core/dataStore/columnStatsStore';
import type { ColumnDistribution, NumericDistribution, StringDistribution } from '@/features/core/dataStore/columnDistributionStore';
import type { DatasetOverview } from '@/features/core/dataStore/datasetOverviewStore';
import Histogram from '@/views/PlotView/Histogram';
import BarChart from '@/views/PlotView/BarChart';
import { Select } from '@/shared/ui';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { DATA_VIEW_ROW_HEIGHT, DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';
import { logger } from '@/utils/appLogger';

export const DataViewWindow: React.FC = () => {
  const dataframes = useDatabaseStore(s => s.databases);
  const statsByDatabase = useColumnStatsStore(s => s.statsByDatabase);
  const distByDatabase = useColumnDistributionStore(s => s.distByDatabase);
  const overviewByDatabase = useDatasetOverviewStore(s => s.overviewByDatabase);
  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [statsLoading, setStatsLoading] = useState(false);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastScrollTop = useRef<number>(0);
  const headerRef = useRef<HTMLTableSectionElement>(null);
  const [headerHeight, setHeaderHeight] = useState(0);

  const CHUNK_SIZE = DATA_VIEW_CHUNK_SIZE;

  useProjectSync();

  useEffect(() => {
    const ids = Object.keys(dataframes);
    if (ids.length > 0) {
      if (!selectedDfId || !dataframes[selectedDfId]) {
        setSelectedDfId(ids[0]);
      }
    } else {
      setSelectedDfId(null);
      setLoadedRows([]);
    }
  }, [dataframes, selectedDfId]);

  useEffect(() => {
    if (selectedDfId) {
      loadInitialRows(selectedDfId);
      loadColumnStats(selectedDfId);
    } else {
      setLoadedRows([]);
    }
  }, [selectedDfId]);

  useEffect(() => {
    if (!selectedDfId) return;
    const df = dataframes[selectedDfId] as Record<string, unknown> | undefined;
    if (!df) return;
    const hasMeta = df.name && Array.isArray(df.columns) && df.columns.length > 0;
    if (hasMeta) return;

    DatabaseService.getDatabaseMeta(selectedDfId)
      .then((meta) => {
        useDatabaseStore.getState().updateDatabase(selectedDfId, {
          name: meta.name,
          columns: meta.columns,
          rowCount: meta.rowCount,
          columnCount: meta.columnCount,
        });
      })
      .catch((e) => logger.data.warn('getDatabaseMeta failed: ' + String(e), 'DataViewWindow'));
  }, [selectedDfId, dataframes]);

  const loadInitialRows = async (id: string) => {
    setLoading(true);
    try {
      const rows = await DatabaseService.getDatabaseRows(id, 0, CHUNK_SIZE);
      setLoadedRows(rows);
    } catch (e) {
      logger.data.error('Failed to load initial rows: ' + String(e), 'DataViewWindow');
    } finally {
      setLoading(false);
    }
  };

  const loadColumnStats = async (id: string) => {
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
  };

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
  }, [selectedDfId, loadingMore, loadedRows.length, dataframes]);

  const refreshData = async () => {
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
          if (scrollRef.current) {
            scrollRef.current.scrollTop = lastScrollTop.current;
          }
        }, 0);
      }
    } catch (e) {
      logger.data.error('Failed to fetch dataframes: ' + String(e), 'DataViewWindow');
    } finally {
      setLoading(false);
    }
  };

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (target.scrollHeight - target.scrollTop - target.clientHeight < 100) {
      loadMoreRows();
    }
  };

  useEffect(() => {
    const el = headerRef.current;
    if (!el) {
      setHeaderHeight(0);
      return;
    }
    setHeaderHeight(el.offsetHeight);
    const ro = new ResizeObserver(() => setHeaderHeight(el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, [selectedDfId]);

  useEffect(() => {
    getCurrentWindow().show().catch((e) => logger.app.error(String(e), 'DataViewWindow'));
    refreshData();

    const setupListeners = async () => {
      const currentWindow = getCurrentWindow();
      const maximized = await currentWindow.isMaximized();
      setIsMaximized(maximized);

      const unlisten = await currentWindow.onResized(async () => {
        const maximized = await currentWindow.isMaximized();
        setIsMaximized(maximized);
      });

      return unlisten;
    };

    let cleanup: (() => void) | null = null;
    setupListeners().then(unlisten => {
      cleanup = unlisten;
    });

    return () => {
      if (cleanup) cleanup();
    };
  }, []);

  const handleMinimize = async () => {
    await getCurrentWindow().minimize();
  };

  const handleMaximize = async () => {
    await getCurrentWindow().toggleMaximize();
  };

  const handleClose = async () => {
    await getCurrentWindow().close();
  };

  const selectedDf = selectedDfId ? dataframes[selectedDfId] : null;
  const columns = ((selectedDf as { columns?: Array<{ name: string; type: string }> })?.columns ?? []);
  const colSpan = columns.length + 1;
  const totalRowCount = (selectedDf as { rowCount?: number })?.rowCount ?? 0;

  const virtualizer = useVirtualizer({
    count: loadedRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => DATA_VIEW_ROW_HEIGHT,
    overscan: 20,
  });

  const virtualItems = virtualizer.getVirtualItems();

  const fmtNum = (v: number | null | undefined, digits = 4) =>
    v == null ? '—' : Number.isInteger(v) ? String(v) : v.toFixed(digits);

  const columnStatsMap = selectedDfId ? statsByDatabase[selectedDfId] : undefined;
  const columnDistMap = selectedDfId ? distByDatabase[selectedDfId] : undefined;

  const currentOverview: DatasetOverview | undefined = selectedDfId ? overviewByDatabase[selectedDfId] : undefined;

  const fmtMemory = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const fmtPercent = (v: number): string => `${(v * 100).toFixed(2)}%`;

  const StatRow: React.FC<{ label: string; value: string | number }> = ({ label, value }) => (
    <>
      <div className="text-gray-500">{label}</div>
      <div className="font-mono text-gray-400 text-right">{value}</div>
    </>
  );

  const OverviewSection: React.FC<{ title: string; icon: React.ReactNode; children: React.ReactNode }> = ({ title, icon, children }) => (
    <div className="flex-1 min-w-0 rounded border border-gray-800 bg-[var(--workbench-bg)]/50 overflow-hidden">
      <div className="flex items-center gap-1.5 px-2.5 py-1 border-b border-gray-800/50">
        {icon}
        <span className="text-[9px] font-bold uppercase tracking-widest text-gray-500">{title}</span>
      </div>
      <div className="px-2.5 py-2">{children}</div>
    </div>
  );

  const RightPanel: React.FC = () => (
    <div className="w-[520px] shrink-0 flex flex-col border-l border-gray-800 bg-[var(--sidebar-bg)] h-full overflow-hidden">
      {/* 上方：Overview */}
      {currentOverview && (() => {
        const { sizeShape, schemaOverview, dataCompleteness } = currentOverview;
        return (
          <div className="shrink-0 border-b border-gray-800">
            <div className="h-8 flex items-center gap-2 px-3 border-b border-gray-800 shrink-0">
              <svg className="w-3.5 h-3.5 text-[var(--accent-color)]" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="2" y="2" width="12" height="12" rx="1" />
                <line x1="2" y1="6" x2="14" y2="6" />
                <line x1="2" y1="10" x2="14" y2="10" />
                <line x1="6" y1="2" x2="6" y2="14" />
              </svg>
              <span className="text-[11px] font-bold uppercase tracking-widest text-gray-500">Overview</span>
              {statsLoading && <span className="text-[9px] text-[var(--accent-color)] animate-pulse ml-auto">loading…</span>}
            </div>
            <div className="p-2.5 flex gap-2">
              <OverviewSection
                title="Size & Shape"
                icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="1" y="1" width="14" height="14" rx="1" /><line x1="8" y1="1" x2="8" y2="15" /><line x1="1" y1="8" x2="15" y2="8" /></svg>}
              >
                <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
                  <StatRow label="n_rows" value={sizeShape.nRows.toLocaleString()} />
                  <StatRow label="n_columns" value={sizeShape.nColumns} />
                  <StatRow label="memory" value={fmtMemory(sizeShape.memorySize)} />
                  <StatRow label="duplicated" value={sizeShape.duplicatedRows.toLocaleString()} />
                </div>
              </OverviewSection>

              <OverviewSection
                title="Schema"
                icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M4 2v12M12 2v12M1 5h14M1 11h14" /></svg>}
              >
                <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
                  <StatRow label="numeric" value={schemaOverview.numericCols} />
                  <StatRow label="categorical" value={schemaOverview.categoricalCols} />
                  <StatRow label="string" value={schemaOverview.stringCols} />
                  <StatRow label="datetime" value={schemaOverview.datetimeCols} />
                  <StatRow label="bool" value={schemaOverview.boolCols} />
                </div>
              </OverviewSection>

              <OverviewSection
                title="Completeness"
                icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="8" cy="8" r="6" /><path d="M8 5v4l2.5 1.5" /></svg>}
              >
                <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
                  <StatRow label="nulls" value={dataCompleteness.totalNulls.toLocaleString()} />
                  <StatRow label="null_ratio" value={fmtPercent(dataCompleteness.nullRatio)} />
                  <StatRow label="null_cols" value={dataCompleteness.colsWithNulls} />
                  <StatRow label="null_rows" value={dataCompleteness.rowsWithNulls.toLocaleString()} />
                </div>
              </OverviewSection>
            </div>
          </div>
        );
      })()}

      {/* 下方：Column Stats */}
      <div className="h-7 flex items-center gap-2 px-3 border-b border-gray-800 shrink-0">
        <VscSymbolNumeric className="text-[var(--accent-color)]" size={13} />
        <span className="text-[11px] font-bold uppercase tracking-widest text-gray-500">Column Stats</span>
        {statsLoading && <span className="text-[9px] text-[var(--accent-color)] animate-pulse ml-auto">loading…</span>}
      </div>
      <OverlayScrollbar className="flex-1 min-h-0">
        <div className="p-2.5 space-y-3">
          {columns.map((col, i) => {
            const stat: ColumnStats | undefined = columnStatsMap?.[col.name];
            const dist: ColumnDistribution | undefined = columnDistMap?.[col.name];
            return (
              <div
                key={i}
                className="rounded border border-gray-800 bg-[var(--workbench-bg)]/50 p-2.5 space-y-1.5"
              >
                <div className="flex items-center gap-2 pb-1.5 border-b border-gray-800/50">
                  <span className="text-[11px] font-bold text-gray-300 truncate flex-1">{col.name}</span>
                  <span className="text-[9px] font-mono text-[var(--accent-color)]/70">{col.type}</span>
                </div>
                {!stat ? (
                  <div className="text-[10px] text-gray-600 italic py-1">
                    {statsLoading ? 'computing…' : 'no data'}
                  </div>
                ) : (
                  <div className="flex gap-3 items-stretch">
                    <div className="w-36 shrink-0">
                      {stat.kind === 'string' ? (
                        <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-[10px]">
                          <div className="text-gray-500">count</div>
                          <div className="font-mono text-gray-400 text-right">{stat.count}</div>
                          <div className="text-gray-500">null_count</div>
                          <div className="font-mono text-gray-400 text-right">{stat.nullCount}</div>
                          <div className="text-gray-500">empty_count</div>
                          <div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).emptyCount}</div>
                          <div className="text-gray-500">valid_ratio</div>
                          <div className="font-mono text-gray-400 text-right">{fmtNum((stat as StringColumnStats).validRatio, 2)}</div>
                          <div className="text-gray-500">unique</div>
                          <div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).unique}</div>
                          <div className="text-gray-500">mode</div>
                          <div className="font-mono text-gray-400 text-right truncate" title={(stat as StringColumnStats).mode ?? ''}>{(stat as StringColumnStats).mode ?? '—'}</div>
                          <div className="text-gray-500">mode_count</div>
                          <div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).modeCount}</div>
                        </div>
                      ) : (
                        <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-[10px]">
                          <div className="text-gray-500">count</div>
                          <div className="font-mono text-gray-400 text-right">{stat.count}</div>
                          <div className="text-gray-500">null_count</div>
                          <div className="font-mono text-gray-400 text-right">{stat.nullCount}</div>
                          <div className="text-gray-500">min</div>
                          <div className="font-mono text-gray-400 text-right truncate">{fmtNum((stat as NumericColumnStats).min)}</div>
                          <div className="text-gray-500">max</div>
                          <div className="font-mono text-gray-400 text-right truncate">{fmtNum((stat as NumericColumnStats).max)}</div>
                          <div className="text-gray-500">mean</div>
                          <div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).mean)}</div>
                          <div className="text-gray-500">median</div>
                          <div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).median)}</div>
                          <div className="text-gray-500">std</div>
                          <div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).std)}</div>
                          <div className="text-gray-500">variance</div>
                          <div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).variance)}</div>
                        </div>
                      )}
                    </div>
                    <div className="flex-1 min-w-0 min-h-0">
                      {dist ? (
                        dist.kind === 'numeric' ? (
                          <Histogram
                            data={(dist as NumericDistribution).bins}
                            compact
                          />
                        ) : (
                          <BarChart
                            data={(dist as StringDistribution).categories}
                            horizontal
                            compact
                          />
                        )
                      ) : (
                        <div className="h-full flex items-center justify-center text-[10px] text-gray-600 italic border border-gray-800/30 rounded">
                          {statsLoading ? 'loading…' : '—'}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </OverlayScrollbar>
    </div>
  );

  return (
    <div className="flex flex-col w-full h-screen bg-[var(--workbench-bg)] text-gray-300 overflow-hidden font-sans">
      {/* 自定义标题栏 */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none shrink-0"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <VscDatabase className="text-[var(--accent-color)]" size={16} />
          <span className="text-white font-bold text-sm tracking-tight">Data Viewer</span>
        </div>

        <div className="flex items-center h-full">
          <button
            onClick={handleMinimize}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
            title="最小化"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
            </svg>
          </button>
          <button
            onClick={handleMaximize}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
            title={isMaximized ? '还原' : '最大化'}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
            </svg>
          </button>
          <button
            onClick={handleClose}
            className="w-12 h-10 flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white transition-colors"
            title="关闭"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <div className="h-12 border-b border-gray-800 flex items-center px-4 gap-4 bg-[var(--sidebar-bg)] shrink-0">
        <div className="w-[240px]">
          <Select
            value={selectedDfId || ''}
            onChange={(val) => setSelectedDfId(val)}
            options={Object.entries(dataframes).map(([id, df]) => {
              const d = df as { name?: string; engine?: { csv?: { path?: string }; parquet?: { path?: string } } };
              let label = d.name;
              if (!label && d.engine?.csv?.path) {
                const p = d.engine.csv.path;
                label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p;
              }
              if (!label && d.engine?.parquet?.path) {
                const p = d.engine.parquet.path;
                label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p;
              }
              return { label: String(label ?? id), value: id };
            })}
          />
        </div>

        <button
          onClick={refreshData}
          className="p-2 hover:bg-[var(--sidebar-bg)] rounded transition-colors text-gray-400 hover:text-white flex items-center gap-2 text-xs font-medium"
          title="Refresh Data"
        >
          <VscRefresh className={loading ? 'animate-spin' : ''} size={16} />
          <span>Refresh</span>
        </button>

        {selectedDf && (
          <div className="ml-auto flex items-center gap-4 text-[10px] font-mono opacity-50">
            <span>COLUMNS: {(selectedDf as { columnCount?: number }).columnCount ?? 0}</span>
            <span>ROWS: {totalRowCount}</span>
          </div>
        )}
      </div>

      {/* Main Content: 左侧表格 + 右侧列统计 */}
      <div className="flex-1 flex min-h-0 overflow-hidden">
        <OverlayScrollbar
          ref={scrollRef}
          onScroll={handleScroll}
          direction="both"
          className="flex-1 min-h-0 bg-[var(--workbench-bg)]"
          scrollbarOffsetTop={headerHeight}
        >
        {selectedDf ? (
          <div className="min-w-full inline-block align-middle">
            <table className="min-w-full border-collapse">
              <thead ref={headerRef} className="sticky top-0 z-10 bg-[var(--sidebar-bg)] border-b border-gray-700">
                <tr>
                  <th className="p-2 text-left text-[10px] font-black uppercase text-gray-500 border-r border-gray-800 w-12 text-center">#</th>
                  {columns.map((col, i) => (
                    <th key={i} className="p-2 text-left border-r border-gray-800 group">
                      <div className="flex flex-col">
                        <span className="text-[11px] font-bold text-gray-300">{col.name}</span>
                        <span className="text-[9px] text-[var(--accent-color)]/60 font-mono">{col.type}</span>
                      </div>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {virtualItems.length > 0 && virtualItems[0].start > 0 && (
                  <tr aria-hidden>
                    <td colSpan={colSpan} style={{ height: virtualItems[0].start, padding: 0, border: 'none' }} />
                  </tr>
                )}
                {virtualItems.map((virtualRow) => {
                  const row = loadedRows[virtualRow.index];
                  if (!row) return null;
                  return (
                    <tr
                      key={virtualRow.key}
                      data-index={virtualRow.index}
                      className="hover:bg-white/[0.02] transition-colors"
                      style={{ height: DATA_VIEW_ROW_HEIGHT }}
                    >
                      <td className="p-2 text-[10px] font-mono text-gray-600 border-r border-gray-800 text-center">{virtualRow.index + 1}</td>
                      {(Array.isArray(row) ? row : []).map((val, j) => (
                        <td key={j} className="p-2 text-[11px] text-gray-400 border-r border-gray-800/50 truncate max-w-[200px]">
                          {val === null ? <span className="italic opacity-30">null</span> : String(val)}
                        </td>
                      ))}
                    </tr>
                  );
                })}
                {virtualItems.length > 0 && (
                  <tr aria-hidden>
                    <td
                      colSpan={colSpan}
                      style={{
                        height: virtualizer.getTotalSize() - (virtualItems[virtualItems.length - 1]?.end ?? 0),
                        padding: 0,
                        border: 'none',
                      }}
                    />
                  </tr>
                )}
              </tbody>
            </table>
            {loadingMore && (
              <div className="p-4 text-center text-xs text-[var(--accent-color)] animate-pulse font-medium">
                Loading more data...
              </div>
            )}
            {totalRowCount > loadedRows.length && !loadingMore && (
              <div className="p-4 text-center text-xs text-gray-500 italic border-t border-gray-800">
                Scroll down to load more (showing {loadedRows.length} of {totalRowCount})
              </div>
            )}
            {totalRowCount <= loadedRows.length && totalRowCount > 0 && (
              <div className="p-4 text-center text-[9px] text-gray-600 uppercase tracking-widest border-t border-gray-800/30">
                End of data
              </div>
            )}
          </div>
        ) : (
          <div className="h-full flex flex-col items-center justify-center opacity-30 gap-4">
            <VscDatabase size={48} />
            <span className="text-sm font-medium tracking-widest uppercase">
              {loading ? 'Loading project data...' : 'No DataFrame Selected'}
            </span>
          </div>
        )}
      </OverlayScrollbar>

        {selectedDf && columns.length > 0 && <RightPanel />}
      </div>
    </div>
  );
};

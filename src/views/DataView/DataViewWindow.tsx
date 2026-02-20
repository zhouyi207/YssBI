import React, { useEffect, useState, useRef, useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DatabaseService } from '@/services/database/databaseService';
import { VscDatabase, VscRefresh } from 'react-icons/vsc';
import { useProjectSync } from '@/features/application/initialization';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { Select } from '@/shared/ui';
import { DATA_VIEW_ROW_HEIGHT, DATA_VIEW_CHUNK_SIZE } from '@/app/appConfig/default';

export const DataViewWindow: React.FC = () => {
  const dataframes = useDatabaseStore(s => s.databases);
  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastScrollTop = useRef<number>(0);

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
      .catch((e) => console.warn('[DataViewWindow] getDatabaseMeta failed:', e));
  }, [selectedDfId, dataframes]);

  const loadInitialRows = async (id: string) => {
    setLoading(true);
    try {
      const rows = await DatabaseService.getDatabaseRows(id, 0, CHUNK_SIZE);
      setLoadedRows(rows);
    } catch (e) {
      console.error('Failed to load initial rows:', e);
    } finally {
      setLoading(false);
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
      console.error('Failed to load more rows:', e);
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

        setTimeout(() => {
          if (scrollRef.current) {
            scrollRef.current.scrollTop = lastScrollTop.current;
          }
        }, 0);
      }
    } catch (e) {
      console.error('Failed to fetch dataframes:', e);
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
    getCurrentWindow().show().catch(console.error);
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

      {/* Main Content */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 min-h-0 overflow-auto bg-[var(--workbench-bg)] custom-scrollbar"
      >
        {selectedDf ? (
          <div className="min-w-full inline-block align-middle">
            <table className="min-w-full border-collapse">
              <thead className="sticky top-0 z-10 bg-[var(--sidebar-bg)] border-b border-gray-700">
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
      </div>
    </div>
  );
};

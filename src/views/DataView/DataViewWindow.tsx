import React, { useEffect, useState, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ProjectService } from '../../services/project/projectService';
import { VscDatabase, VscRefresh } from 'react-icons/vsc';
import { useProjectStore } from '@/views/EditorView/Store/useProjectStore';
import { useProjectSync, initProjectSync } from '@/views/EditorView/Hooks/useProjectSync';
import { Select } from '@/views/EditorView/Shared/UI/Select';

export const DataViewWindow: React.FC = () => {
  const dataframes = useProjectStore(s => s.dataframes);
  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [loadedRows, setLoadedRows] = useState<any[][]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastScrollTop = useRef<number>(0);

  const CHUNK_SIZE = 100;

  // 启用自动同步，监听后端 DataFrame 更改
  useProjectSync({
    enabled: true,
  });

  // 自动管理选中状态
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

  // 当选中的 DataFrame 变化时，重置并加载首屏数据
  useEffect(() => {
    if (selectedDfId) {
      loadInitialRows(selectedDfId);
    } else {
      setLoadedRows([]);
    }
  }, [selectedDfId]);

  const loadInitialRows = async (id: string) => {
    setLoading(true);
    try {
      const rows = await ProjectService.getDataFrameRows(id, 0, CHUNK_SIZE);
      setLoadedRows(rows);
    } catch (e) {
      console.error('Failed to load initial rows:', e);
    } finally {
      setLoading(false);
    }
  };

  const loadMoreRows = async () => {
    if (!selectedDfId || loadingMore) return;
    const currentCount = loadedRows.length;
    const totalCount = dataframes[selectedDfId]?.rowCount || 0;

    if (currentCount >= totalCount) return;

    setLoadingMore(true);
    try {
      const newRows = await ProjectService.getDataFrameRows(selectedDfId, currentCount, CHUNK_SIZE);
      setLoadedRows(prev => [...prev, ...newRows]);
    } catch (e) {
      console.error('Failed to load more rows:', e);
    } finally {
      setLoadingMore(false);
    }
  };

  const refreshData = async () => {
    if (scrollRef.current) {
      lastScrollTop.current = scrollRef.current.scrollTop;
    }
    setLoading(true);
    try {
      await initProjectSync();
      if (selectedDfId) {
        const rows = await ProjectService.getDataFrameRows(selectedDfId, 0, Math.max(loadedRows.length, CHUNK_SIZE));
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

  return (
    <div className="flex flex-col w-full h-screen bg-[var(--workbench-bg)] text-gray-300 overflow-hidden font-sans">
      {/* 自定义标题栏 - 与主窗口一致 */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none shrink-0"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <VscDatabase className="text-[var(--accent-color)]" size={16} />
          <span className="text-white font-bold text-sm tracking-tight">Data Viewer</span>
        </div>

        {/* 窗口控制按钮 */}
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
            options={Object.entries(dataframes).map(([id, df]) => ({
              label: df.name,
              value: id
            }))}
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
            <span>COLUMNS: {selectedDf.columnCount}</span>
            <span>ROWS: {selectedDf.rowCount}</span>
          </div>
        )}
      </div>

      {/* Main Content */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-auto bg-[var(--workbench-bg)] custom-scrollbar"
      >
        {selectedDf ? (
          <div className="min-w-full inline-block align-middle">
            <table className="min-w-full border-collapse">
              <thead className="sticky top-0 z-10 bg-[var(--sidebar-bg)] border-b border-gray-700">
                <tr>
                  <th className="p-2 text-left text-[10px] font-black uppercase text-gray-500 border-r border-gray-800 w-12 text-center">#</th>
                  {selectedDf.columns.map((col, i) => (
                    <th key={i} className="p-2 text-left border-r border-gray-800 group">
                      <div className="flex flex-col">
                        <span className="text-[11px] font-bold text-gray-300">{col.name}</span>
                        <span className="text-[9px] text-[var(--accent-color)]/60 font-mono">{col.type}</span>
                      </div>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-800/50">
                {loadedRows.map((row, i) => (
                  <tr key={i} className="hover:bg-white/[0.02] transition-colors">
                    <td className="p-2 text-[10px] font-mono text-gray-600 border-r border-gray-800 text-center">{i + 1}</td>
                    {row.map((val, j) => (
                      <td key={j} className="p-2 text-[11px] text-gray-400 border-r border-gray-800/50 truncate max-w-[200px]">
                        {val === null ? <span className="italic opacity-30">null</span> : String(val)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
            {loadingMore && (
              <div className="p-4 text-center text-xs text-[var(--accent-color)] animate-pulse font-medium">
                Loading more data...
              </div>
            )}
            {selectedDf.rowCount > loadedRows.length && !loadingMore && (
              <div className="p-4 text-center text-xs text-gray-500 italic border-t border-gray-800">
                Scroll down to load more (showing {loadedRows.length} of {selectedDf.rowCount})
              </div>
            )}
            {selectedDf.rowCount <= loadedRows.length && selectedDf.rowCount > 0 && (
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
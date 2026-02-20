import { useEffect, useRef, useState, useCallback } from 'react';
import { createPortal } from 'react-dom';

const LOGS_DRAG_TYPE = 'application/x-yssbi-logs-drag';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useLogStore } from '@/features/core/log/logStore';
import { LogMessage, LogLevel, LogType } from '@/shared/types/ui';
import { FiTrash2, FiFilter, FiSearch, FiChevronDown, FiChevronUp, FiX } from 'react-icons/fi';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { uiStore } from '@/features/core/ui/UIStore';

export interface LogPanelContentProps {
  variant?: 'embedded' | 'standalone';
  className?: string;
}

export const LogPanelContent = ({ variant = 'embedded', className = '' }: LogPanelContentProps) => {
  const logs = useLogStore((s) => s.logs);
  const filter = useLogStore((s) => s.filter);
  const addLog = useLogStore((s) => s.addLog);
  const clearLogs = useLogStore((s) => s.clearLogs);
  const toggleLevel = useLogStore((s) => s.toggleLevel);
  const toggleType = useLogStore((s) => s.toggleType);
  const setSearchText = useLogStore((s) => s.setSearchText);
  const getFilteredLogs = useLogStore((s) => s.getFilteredLogs);
  const loadLogs = useLogStore((s) => s.loadLogs);
  const loadMoreLogs = useLogStore((s) => s.loadMoreLogs);
  const refreshLogs = useLogStore((s) => s.refreshLogs);
  const loading = useLogStore((s) => s.loading);
  const hasMore = useLogStore((s) => s.hasMore);
  const total = useLogStore((s) => s.total);
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const filterButtonRef = useRef<HTMLButtonElement>(null);
  const filterPopoverRef = useRef<HTMLDivElement>(null);
  const [popoverPosition, setPopoverPosition] = useState({ top: 0, left: 0 });
  const [isInitialLoad, setIsInitialLoad] = useState(true);

  useEffect(() => {
    loadLogs(0, 100).then(() => {
      setIsInitialLoad(false);
      setTimeout(() => {
        if (logContainerRef.current) {
          logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
        }
      }, 100);
    });

    const unlisten = listen<LogMessage>('log-message', (event) => {
      addLog(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (autoScroll && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  useEffect(() => {
    if (!isFilterOpen || !filterButtonRef.current) return;
    const rect = filterButtonRef.current.getBoundingClientRect();
    setPopoverPosition({ top: rect.bottom + 4, left: rect.right - 280 });
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        filterButtonRef.current?.contains(target) ||
        filterPopoverRef.current?.contains(target)
      ) return;
      setIsFilterOpen(false);
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isFilterOpen]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (target.scrollTop < 100 && hasMore && !loading) {
      const scrollHeight = target.scrollHeight;
      const scrollTop = target.scrollTop;

      loadMoreLogs().then(() => {
        if (logContainerRef.current) {
          const newScrollHeight = logContainerRef.current.scrollHeight;
          const heightDiff = newScrollHeight - scrollHeight;
          logContainerRef.current.scrollTop = scrollTop + heightDiff;
        }
      });
    }
  };

  const openInNewWindow = useCallback(async (x?: number, y?: number) => {
    try {
      const label = `logs-${Math.random().toString(36).substring(7)}`;
      const opts: Record<string, unknown> = {
        url: 'index.html#/logs',
        title: 'Logs',
        width: 1000,
        height: 600,
        decorations: false,
        visible: false,
      };
      if (typeof x === 'number' && typeof y === 'number') {
        opts.x = x;
        opts.y = y;
      }
      new WebviewWindow(label, opts);
    } catch (error) {
      console.error('Failed to open logs window:', error);
      uiStore.showToast('无法打开日志窗口', 'error');
    }
  }, []);

  const dragImageRef = useRef<HTMLDivElement>(null);
  const droppedOnOurWindowRef = useRef(false);
  const lastDragPosRef = useRef<{ x: number; y: number } | null>(null);

  const handleEmbeddedDragStart = useCallback((e: React.DragEvent) => {
    if (variant !== 'embedded') return;
    droppedOnOurWindowRef.current = false;
    lastDragPosRef.current = { x: e.screenX, y: e.screenY };
    e.dataTransfer.setData(LOGS_DRAG_TYPE, '');
    e.dataTransfer.effectAllowed = 'move';
    if (dragImageRef.current) {
      e.dataTransfer.setDragImage(dragImageRef.current, 0, 0);
    }
  }, [variant]);

  const handleEmbeddedDragEnd = useCallback(async (e: React.DragEvent) => {
    if (variant !== 'embedded') return;
    const last = lastDragPosRef.current;
    lastDragPosRef.current = null;
    if (!droppedOnOurWindowRef.current) {
      // 在窗口外释放：droppedOnOurWindowRef 为 false 即表示未在本窗口内 drop，直接创建新窗口
      // 优先用 dragend 的 screenX/screenY（部分环境会提供释放位置），否则用最后 dragover 位置
      const sx = e.screenX ?? 0;
      const sy = e.screenY ?? 0;
      const pos = (sx !== 0 || sy !== 0) ? { x: sx, y: sy } : (last ?? { x: 100, y: 100 });
      try {
        openInNewWindow(pos.x, pos.y);
      } catch (err) {
        console.error('Failed to open logs window:', err);
      }
    }
  }, [variant, openInNewWindow]);

  useEffect(() => {
    if (variant !== 'embedded') return;
    const onDragOver = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes(LOGS_DRAG_TYPE)) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
        lastDragPosRef.current = { x: e.screenX, y: e.screenY };
      }
    };
    const onDrop = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes(LOGS_DRAG_TYPE)) {
        e.preventDefault();
        droppedOnOurWindowRef.current = true;
      }
    };
    document.addEventListener('dragover', onDragOver);
    document.addEventListener('drop', onDrop);
    return () => {
      document.removeEventListener('dragover', onDragOver);
      document.removeEventListener('drop', onDrop);
    };
  }, [variant]);

  const panelNode = useLayoutStore((s) => s.nodes['panel']);
  const updateNode = useLayoutStore((s) => s.updateNode);

  const handleClose = useCallback(() => {
    if (variant === 'embedded') {
      updateNode('panel', {
        data: { ...panelNode?.data, visible: false },
      });
    } else {
      getCurrentWindow().close();
    }
  }, [variant, panelNode?.data, updateNode]);

  const filteredLogs = getFilteredLogs() ?? [];
  const safeLogs = logs ?? [];

  const getLevelColor = (level: LogLevel) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warn': return 'text-yellow-400';
      case 'info': return 'text-blue-400';
      case 'debug': return 'text-gray-400';
      case 'trace': return 'text-gray-500';
      default: return 'text-gray-400';
    }
  };

  const getLevelBgColor = (level: LogLevel) => {
    switch (level) {
      case 'error': return 'bg-red-500/10';
      case 'warn': return 'bg-yellow-500/10';
      case 'info': return 'bg-blue-500/10';
      case 'debug': return 'bg-gray-500/10';
      case 'trace': return 'bg-gray-600/10';
      default: return 'bg-gray-500/10';
    }
  };

  const getTypeColor = (type: LogType) => {
    switch (type) {
      case 'application': return 'text-green-400';
      case 'execution': return 'text-purple-400';
      case 'system': return 'text-cyan-400';
      default: return 'text-gray-400';
    }
  };

  return (
    <div className={`flex flex-col h-full bg-[var(--workbench-bg)] text-white overflow-hidden ${className}`}>
      {/* 拖拽预览图 - 供 setDragImage 使用，由系统渲染，可在窗口外显示 */}
      {variant === 'embedded' &&
        createPortal(
          <div
            ref={dragImageRef}
            className="fixed -left-[9999px] -top-[9999px] w-64 rounded-lg border-2 border-[var(--accent-color)]/60 bg-[var(--workbench-bg)] overflow-hidden opacity-95 shadow-2xl pointer-events-none select-none"
            aria-hidden
          >
            <div className="flex items-center gap-2 px-3 py-2 bg-[var(--sidebar-bg)] border-b border-gray-700">
              <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <span className="text-xs font-semibold text-white">Logs</span>
              <span className="text-[10px] text-gray-400 ml-1">松开创建新窗口</span>
            </div>
            <div className="h-16 bg-[var(--workbench-bg)] flex items-center justify-center">
              <span className="text-xs text-gray-500">拖到窗口外释放</span>
            </div>
          </div>,
          document.body
        )}

      {/* 工具栏 */}
      <div
        className={`flex items-center justify-between px-3 py-2 bg-[var(--sidebar-bg)] border-b border-gray-800 shrink-0 ${variant === 'embedded' ? 'select-none cursor-grab active:cursor-grabbing' : ''}`}
        {...(variant === 'embedded' && {
          draggable: true,
          onDragStart: handleEmbeddedDragStart,
          onDragEnd: handleEmbeddedDragEnd,
          title: '拖动到窗口外可打开新窗口',
        })}
      >
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${loading ? 'bg-yellow-500 animate-pulse' : 'bg-green-500 animate-pulse'}`}></div>
            <span className="text-xs font-medium text-gray-300">
              {loading ? '加载中...' : '日志监控'}
            </span>
          </div>
          <div className="h-4 w-px bg-gray-700"></div>
          <span className="text-xs text-gray-400">
            显示 <span className="text-[var(--accent-color)] font-semibold">{filteredLogs.length}</span> / {total} 条
          </span>
        </div>
        <div className="flex items-center gap-1 shrink-0" onPointerDown={(e) => e.stopPropagation()} onMouseDown={(e) => e.stopPropagation()}>
          <button
            onClick={() => refreshLogs()}
            disabled={loading}
            className="px-2 py-1 rounded text-xs font-medium text-gray-400 hover:bg-gray-500/20 transition-colors disabled:opacity-50"
            title="刷新日志"
          >
            <svg className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </button>
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={`px-2 py-1 rounded text-xs font-medium transition-colors ${autoScroll
              ? 'text-[var(--accent-color)] hover:bg-[var(--accent-color)]/20'
              : 'text-gray-400 hover:bg-gray-500/20'
              }`}
            title={autoScroll ? '自动滚动已启用' : '自动滚动已禁用'}
          >
            {autoScroll ? <FiChevronDown size={14} /> : <FiChevronUp size={14} />}
          </button>
          <div className="relative">
            <button
              ref={filterButtonRef}
              onClick={() => setIsFilterOpen(!isFilterOpen)}
              className={`px-2 py-1 rounded text-xs font-medium transition-colors ${isFilterOpen
                ? 'text-[var(--accent-color)] hover:bg-[var(--accent-color)]/20'
                : 'text-gray-400 hover:bg-gray-500/20'
                }`}
              title="过滤器"
            >
              <FiFilter size={14} />
            </button>
            {isFilterOpen &&
              createPortal(
                <div
                  ref={filterPopoverRef}
                  className="fixed z-[9999] w-[280px] p-3 rounded-lg shadow-2xl border border-gray-700 bg-[var(--workbench-bg)] space-y-3"
                  style={{ top: popoverPosition.top, left: popoverPosition.left }}
                  onClick={(e) => e.stopPropagation()}
                >
                <div className="relative">
                  <FiSearch className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" size={14} />
                  <input
                    type="text"
                    placeholder="搜索日志内容..."
                    value={filter?.searchText ?? ''}
                    onChange={(e) => setSearchText(e.target.value)}
                    className="w-full pl-9 pr-3 py-2 text-sm bg-[var(--sidebar-bg)] border border-gray-700 rounded-md focus:outline-none focus:border-[var(--accent-color)] focus:ring-1 focus:ring-[var(--accent-color)]/50 text-gray-200 placeholder-gray-500 transition-all"
                  />
                </div>
                <div>
                  <div className="text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider">级别</div>
                  <div className="flex flex-wrap gap-2">
                    {(['error', 'warn', 'info', 'debug', 'trace'] as LogLevel[]).map((level) => (
                      <button
                        key={level}
                        onClick={() => toggleLevel(level)}
                        className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${filter?.levels?.has(level)
                          ? `${getLevelBgColor(level)} ${getLevelColor(level)} border border-current`
                          : 'bg-[var(--sidebar-bg)] text-gray-500 border border-transparent hover:border-gray-600'
                          }`}
                      >
                        {level.toUpperCase()}
                      </button>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider">类型</div>
                  <div className="flex flex-wrap gap-2">
                    {(['application', 'execution', 'system'] as LogType[]).map((type) => (
                      <button
                        key={type}
                        onClick={() => toggleType(type)}
                        className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${filter?.types?.has(type)
                          ? `bg-gray-700 ${getTypeColor(type)} border border-current`
                          : 'bg-[var(--sidebar-bg)] text-gray-500 border border-transparent hover:border-gray-600'
                          }`}
                      >
                        {type === 'application' ? '应用' : type === 'execution' ? '执行' : '系统'}
                      </button>
                    ))}
                  </div>
                </div>
              </div>,
                document.body
              )}
          </div>
          <button
            onClick={clearLogs}
            className="px-2 py-1 rounded text-xs font-medium text-red-400 hover:bg-red-500/20 transition-colors"
            title="清空日志"
          >
            <FiTrash2 size={14} />
          </button>
          <button
            onClick={handleClose}
            className="px-2 py-1 rounded text-xs font-medium text-gray-400 hover:bg-gray-500/20 transition-colors"
            title={variant === 'embedded' ? '关闭日志面板' : '关闭窗口'}
          >
            <FiX size={14} />
          </button>
        </div>
      </div>

      {/* 日志列表 */}
      <OverlayScrollbar
        ref={logContainerRef}
        onScroll={handleScroll}
        className="flex-1 bg-[var(--workbench-bg)]"
        direction="vertical"
      >
        {isInitialLoad ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 gap-3">
            <div className="w-8 h-8 border-2 border-[var(--accent-color)] border-t-transparent rounded-full animate-spin"></div>
            <div className="text-sm">加载日志中...</div>
          </div>
        ) : (filteredLogs?.length ?? 0) === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 gap-3">
            <svg className="w-16 h-16 opacity-30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <div className="text-center">
              <div className="text-sm font-medium mb-1">
                {safeLogs.length === 0 ? '暂无日志' : '没有匹配的日志'}
              </div>
              <div className="text-xs opacity-60">
                {safeLogs.length === 0 ? '执行图后将显示日志' : '尝试调整过滤条件'}
              </div>
            </div>
          </div>
        ) : (
          <div className="p-3">
            {loading && (
              <div className="text-center py-2 text-xs text-[var(--accent-color)] flex items-center justify-center gap-2">
                <div className="w-3 h-3 border-2 border-[var(--accent-color)] border-t-transparent rounded-full animate-spin"></div>
                加载中...
              </div>
            )}
            <div className="space-y-1">
              {filteredLogs.map((log, index) => (
                <div
                  key={index}
                  className={`flex gap-3 px-3 py-2 rounded-md hover:bg-[var(--sidebar-bg)] transition-colors ${getLevelBgColor(log.level)} border-l-2 ${log.level === 'error' ? 'border-red-500' :
                    log.level === 'warn' ? 'border-yellow-500' :
                      log.level === 'info' ? 'border-blue-500' :
                        log.level === 'debug' ? 'border-gray-500' :
                          'border-gray-600'
                    }`}
                >
                  <span className="text-gray-500 shrink-0 text-[11px] font-mono">
                    {log.timestamp.split(' ')[1]}
                  </span>
                  <span className={`${getLevelColor(log.level)} font-bold shrink-0 w-14 text-[10px] uppercase`}>
                    {log.level}
                  </span>
                  <span className={`${getTypeColor(log.log_type)} shrink-0 text-[10px] font-semibold px-1.5 py-0.5 rounded ${log.log_type === 'application' ? 'bg-green-500/10' :
                    log.log_type === 'execution' ? 'bg-purple-500/10' :
                      'bg-cyan-500/10'
                    }`}>
                    {log.log_type === 'application' ? 'APP' : log.log_type === 'execution' ? 'EXEC' : 'SYS'}
                  </span>
                  {log.source && (
                    <span className="text-cyan-400 shrink-0 text-[11px] font-mono opacity-70">
                      [{log.source}]
                    </span>
                  )}
                  <span className="text-gray-200 break-all text-[11px] leading-relaxed font-mono flex-1">
                    {log.message}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </OverlayScrollbar>
    </div>
  );
};

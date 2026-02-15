import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useLogStore } from '@/features/core/log/logStore';
import { LogMessage, LogLevel, LogType } from '@/shared/types/ui';
import { FiTrash2, FiFilter, FiSearch, FiChevronDown, FiChevronUp } from 'react-icons/fi';

export const LogWindow = () => {
  const { logs, filter, addLog, clearLogs, toggleLevel, toggleType, setSearchText, getFilteredLogs, loadLogs, loadMoreLogs, refreshLogs, loading, hasMore, total } = useLogStore();
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [isMaximized, setIsMaximized] = useState(false);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const [isInitialLoad, setIsInitialLoad] = useState(true);

  // 显示窗口并初始化
  useEffect(() => {
    let cleanup: (() => void) | null = null;

    const initWindow = async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const currentWindow = getCurrentWindow();
      
      // 立即显示窗口
      await currentWindow.show().catch(console.error);

      // 监听窗口最大化状态
      const maximized = await currentWindow.isMaximized();
      setIsMaximized(maximized);

      const unlisten = await currentWindow.onResized(async () => {
        const maximized = await currentWindow.isMaximized();
        setIsMaximized(maximized);
      });

      cleanup = unlisten;
    };

    initWindow();

    // 加载初始日志（最新的 100 条）
    loadLogs(0, 100).then(() => {
      setIsInitialLoad(false);
      // 加载完成后滚动到底部
      setTimeout(() => {
        if (logContainerRef.current) {
          logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
        }
      }, 100);
    });

    // 监听实时日志事件
    const unlisten = listen<LogMessage>('log-message', (event) => {
      addLog(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
      if (cleanup) cleanup();
    };
  }, []);

  // 自动滚动到底部
  useEffect(() => {
    if (autoScroll && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);
  
  // 滚动加载更多
  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    // 滚动到顶部时加载更多历史日志
    if (target.scrollTop < 100 && hasMore && !loading) {
      const scrollHeight = target.scrollHeight;
      const scrollTop = target.scrollTop;
      
      loadMoreLogs().then(() => {
        // 保持滚动位置（相对于新内容）
        if (logContainerRef.current) {
          const newScrollHeight = logContainerRef.current.scrollHeight;
          const heightDiff = newScrollHeight - scrollHeight;
          logContainerRef.current.scrollTop = scrollTop + heightDiff;
        }
      });
    }
  };

  const filteredLogs = getFilteredLogs();

  // 窗口控制函数
  const handleMinimize = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().minimize();
  };

  const handleMaximize = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().toggleMaximize();
  };

  const handleClose = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  };

  // 获取日志级别的颜色
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

  // 获取日志级别的背景色
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

  // 获取日志类型的颜色
  const getTypeColor = (type: LogType) => {
    switch (type) {
      case 'application': return 'text-green-400';
      case 'execution': return 'text-purple-400';
      case 'system': return 'text-cyan-400';
      default: return 'text-gray-400';
    }
  };

  return (
    <div className="flex flex-col h-screen bg-[var(--workbench-bg)] text-white overflow-hidden">
      {/* 自定义标题栏 - 与主窗口一致 */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none shrink-0 rounded-tr-lg overflow-hidden"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span className="text-white font-bold text-sm tracking-tight">Logs</span>
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

      {/* 工具栏 */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[var(--sidebar-bg)] border-b border-gray-800 shrink-0">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${loading ? 'bg-yellow-500 animate-pulse' : 'bg-green-500 animate-pulse'}`}></div>
            <span className="text-xs font-medium text-gray-300">
              {loading ? '加载中...' : '日志监控'}
            </span>
          </div>
          <div className="h-4 w-px bg-gray-700"></div>
          <span className="text-xs text-gray-400">
            显示 <span className="text-[var(--accent-color)] font-semibold">{filteredLogs.length}</span> / {total} 条
            {hasMore && <span className="text-gray-500 ml-1">(还有更多)</span>}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => refreshLogs()}
            disabled={loading}
            className="px-2 py-1 rounded text-xs font-medium text-gray-400 hover:bg-[var(--sidebar-bg)] transition-colors disabled:opacity-50"
            title="刷新日志"
          >
            <svg className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </button>
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={`px-2 py-1 rounded text-xs font-medium transition-colors ${
              autoScroll 
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)] hover:bg-[var(--accent-color)]/30' 
                : 'text-gray-400 hover:bg-[var(--sidebar-bg)]'
            }`}
            title={autoScroll ? '自动滚动已启用' : '自动滚动已禁用'}
          >
            {autoScroll ? <FiChevronDown size={14} /> : <FiChevronUp size={14} />}
          </button>
          <button
            onClick={() => setIsFilterOpen(!isFilterOpen)}
            className={`px-2 py-1 rounded text-xs font-medium transition-colors ${
              isFilterOpen 
                ? 'bg-[var(--accent-color)]/20 text-[var(--accent-color)] hover:bg-[var(--accent-color)]/30' 
                : 'text-gray-400 hover:bg-[var(--sidebar-bg)]'
            }`}
            title="过滤器"
          >
            <FiFilter size={14} />
          </button>
          <button
            onClick={clearLogs}
            className="px-2 py-1 rounded text-xs font-medium text-red-400 hover:bg-red-500/20 transition-colors"
            title="清空日志"
          >
            <FiTrash2 size={14} />
          </button>
        </div>
      </div>

      {/* 过滤器面板 */}
      {isFilterOpen && (
        <div className="px-4 py-3 bg-[var(--workbench-bg)] border-b border-gray-800 space-y-3 shrink-0">
          {/* 搜索框 */}
          <div className="relative">
            <FiSearch className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" size={14} />
            <input
              type="text"
              placeholder="搜索日志内容..."
              value={filter.searchText}
              onChange={(e) => setSearchText(e.target.value)}
              className="w-full pl-9 pr-3 py-2 text-sm bg-[var(--sidebar-bg)] border border-gray-700 rounded-md focus:outline-none focus:border-[var(--accent-color)] focus:ring-1 focus:ring-[var(--accent-color)]/50 text-gray-200 placeholder-gray-500 transition-all"
            />
          </div>

          {/* 日志级别过滤 */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider">级别</div>
            <div className="flex flex-wrap gap-2">
              {(['error', 'warn', 'info', 'debug', 'trace'] as LogLevel[]).map((level) => (
                <button
                  key={level}
                  onClick={() => toggleLevel(level)}
                  className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                    filter.levels.has(level)
                      ? `${getLevelBgColor(level)} ${getLevelColor(level)} border border-current`
                      : 'bg-[var(--sidebar-bg)] text-gray-500 border border-transparent hover:border-gray-600'
                  }`}
                >
                  {level.toUpperCase()}
                </button>
              ))}
            </div>
          </div>

          {/* 日志类型过滤 */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider">类型</div>
            <div className="flex flex-wrap gap-2">
              {(['application', 'execution', 'system'] as LogType[]).map((type) => (
                <button
                  key={type}
                  onClick={() => toggleType(type)}
                  className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                    filter.types.has(type)
                      ? `bg-gray-700 ${getTypeColor(type)} border border-current`
                      : 'bg-[var(--sidebar-bg)] text-gray-500 border border-transparent hover:border-gray-600'
                  }`}
                >
                  {type === 'application' ? '应用' : type === 'execution' ? '执行' : '系统'}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 日志列表 */}
      <div
        ref={logContainerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto bg-[var(--workbench-bg)] custom-scrollbar"
        style={{ minHeight: 0 }}
      >
        {isInitialLoad ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 gap-3">
            <div className="w-8 h-8 border-2 border-[var(--accent-color)] border-t-transparent rounded-full animate-spin"></div>
            <div className="text-sm">加载日志中...</div>
          </div>
        ) : filteredLogs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 gap-3">
            <svg className="w-16 h-16 opacity-30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <div className="text-center">
              <div className="text-sm font-medium mb-1">
                {logs.length === 0 ? '暂无日志' : '没有匹配的日志'}
              </div>
              <div className="text-xs opacity-60">
                {logs.length === 0 ? '执行图后将显示日志' : '尝试调整过滤条件'}
              </div>
            </div>
          </div>
        ) : (
          <div className="p-3">
            {hasMore && !loading && (
              <div className="text-center py-2 text-xs text-gray-500">
                向上滚动加载更多历史日志
              </div>
            )}
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
                  className={`flex gap-3 px-3 py-2 rounded-md hover:bg-[var(--sidebar-bg)] transition-colors ${getLevelBgColor(log.level)} border-l-2 ${
                    log.level === 'error' ? 'border-red-500' :
                    log.level === 'warn' ? 'border-yellow-500' :
                    log.level === 'info' ? 'border-blue-500' :
                    log.level === 'debug' ? 'border-gray-500' :
                    'border-gray-600'
                  }`}
                >
                  {/* 时间戳 */}
                  <span className="text-gray-500 shrink-0 text-[11px] font-mono">
                    {log.timestamp.split(' ')[1]}
                  </span>

                  {/* 级别 */}
                  <span className={`${getLevelColor(log.level)} font-bold shrink-0 w-14 text-[10px] uppercase`}>
                    {log.level}
                  </span>

                  {/* 类型 */}
                  <span className={`${getTypeColor(log.log_type)} shrink-0 text-[10px] font-semibold px-1.5 py-0.5 rounded ${
                    log.log_type === 'application' ? 'bg-green-500/10' :
                    log.log_type === 'execution' ? 'bg-purple-500/10' :
                    'bg-cyan-500/10'
                  }`}>
                    {log.log_type === 'application' ? 'APP' : log.log_type === 'execution' ? 'EXEC' : 'SYS'}
                  </span>

                  {/* 来源 */}
                  {log.source && (
                    <span className="text-cyan-400 shrink-0 text-[11px] font-mono opacity-70">
                      [{log.source}]
                    </span>
                  )}

                  {/* 消息 */}
                  <span className="text-gray-200 break-all text-[11px] leading-relaxed font-mono flex-1">
                    {log.message}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
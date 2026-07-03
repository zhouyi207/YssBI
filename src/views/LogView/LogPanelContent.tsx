import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import { LOGS_DRAG_TYPE, LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from '@/app/appConfig/default';
import { listen } from '@tauri-apps/api/event';
import { openLogsWindow } from '@/features/application/window';
import { useLogStore, applyLogFilter } from '@/features/core/log/logStore';
import { logBuffer } from '@/features/core/log/logBuffer';
import { useLiveLogs } from '@/features/core/log/useLiveLogs';
import { useLogActions } from '@/features/application/log';
import { LogMessage, LogLevel, LogType } from '@/shared/types/ui';
import { FiTrash2, FiFilter, FiSearch, FiChevronDown, FiChevronUp, FiX } from 'react-icons/fi';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '@/features/core/editor';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { logger } from '@/utils/appLogger';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import {
  getLogLevelBackground,
  getLogLevelColor,
  getLogTypeColor,
  LOG_TYPE_BACKGROUND,
  LOG_TYPE_LABELS,
} from './logPresentation';

/** 判定「贴底」的容差（px）：用于自动滚动跟随 */
const SCROLL_BOTTOM_THRESHOLD = 80;

// ─── Log Item Row ───

interface LogItemProps {
  log: LogMessage;
  isSelected: boolean;
  onClick: () => void;
  getLevelColor: (level: LogLevel) => string;
  getLevelBgColor: (level: LogLevel) => string;
  getTypeColor: (type: LogType) => string;
}

const LogItem = ({ log, isSelected, onClick, getLevelColor, getLevelBgColor, getTypeColor }: LogItemProps) => {
  const borderColor = log.level === 'error' ? 'border-red-500'
    : log.level === 'warn' ? 'border-yellow-500'
    : log.level === 'info' ? 'border-blue-500'
    : log.level === 'debug' ? 'border-muted-foreground/50'
    : 'border-muted-foreground/40';

  return (
    <div
      onClick={onClick}
      className={`flex gap-3 px-3 py-2 rounded-md cursor-pointer transition-colors border-l-2 ${borderColor} ${getLevelBgColor(log.level)} ${
        isSelected ? 'ring-1 ring-[var(--accent-color)] bg-[var(--accent-color)]/10' : 'hover:bg-[var(--sidebar-bg)]'
      }`}
    >
      <span className="text-muted-foreground shrink-0 text-[11px] font-mono">{log.timestamp.split(' ')[1]}</span>
      <span className={`${getLevelColor(log.level)} font-bold shrink-0 w-14 text-[10px] uppercase`}>{log.level}</span>
      <span className={`${getTypeColor(log.log_type)} shrink-0 text-[10px] font-semibold px-1.5 py-0.5 rounded ${LOG_TYPE_BACKGROUND[log.log_type] ?? 'bg-muted/50'}`}>
        {LOG_TYPE_LABELS[log.log_type] ?? log.log_type.toUpperCase()}
      </span>
      {log.source && <span className="text-sky-500 shrink-0 text-[11px] font-mono opacity-80">[{log.source}]</span>}
      <span className="text-foreground text-[11px] leading-relaxed font-mono flex-1 min-w-0 truncate">{log.message}</span>
    </div>
  );
};

// ─── Main Component ───

export interface LogPanelContentProps {
  variant?: 'embedded' | 'standalone';
  className?: string;
}

export const LogPanelContent = ({ variant = 'embedded', className = '' }: LogPanelContentProps) => {
  const { t } = useTranslation();
  // 高频日志数据来自 React 之外的 logBuffer（帧级合并），冷控制状态来自 zustand
  const { entries: logs, total, hasMore, loading } = useLiveLogs();
  const filter = useLogStore((s) => s.filter);
  const toggleLevel = useLogStore((s) => s.toggleLevel);
  const toggleType = useLogStore((s) => s.toggleType);
  const setSearchText = useLogStore((s) => s.setSearchText);
  const selectedLog = useLogStore((s) => s.selectedLog);
  const setSelectedLog = useLogStore((s) => s.setSelectedLog);
  const { loadLogs, loadMoreLogs, refreshLogs } = useLogActions();
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  // 用户是否「贴底」——由真实滚动事件维护，而非内容追加后的事后测量。
  // 日志按帧合并后，一帧可能新增大量行（高度跳变远超阈值），事后测量会误判为「离底」。
  const pinnedToBottomRef = useRef(true);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const filterButtonRef = useRef<HTMLButtonElement>(null);
  const filterPopoverRef = useRef<HTMLDivElement>(null);
  const [popoverPosition, setPopoverPosition] = useState({ top: 0, left: 0 });
  const [isInitialLoad, setIsInitialLoad] = useState(true);
  const loadMoreStateRef = useRef({ hasMore, loading, loadMoreLogs });
  loadMoreStateRef.current = { hasMore, loading, loadMoreLogs };

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
      logBuffer.pushLive(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const el = logContainerRef.current;
    if (!autoScroll || !el) return;
    // 只要在「贴底」状态就持续跟随，无论本帧新增多少行
    if (!pinnedToBottomRef.current) return;
    el.scrollTop = el.scrollHeight;
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
    return addGlobalEventListener(document, 'mousedown', handleClickOutside);
  }, [isFilterOpen]);

  useEffect(() => {
    if (isInitialLoad) return;
    const el = logContainerRef.current;
    if (!el) return;
    const onScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = el;
      pinnedToBottomRef.current = scrollHeight - scrollTop - clientHeight < SCROLL_BOTTOM_THRESHOLD;
      const { hasMore, loading, loadMoreLogs } = loadMoreStateRef.current;
      if (el.scrollTop < 150 && hasMore && !loading) {
        const scrollHeight = el.scrollHeight;
        const scrollTop = el.scrollTop;
        loadMoreLogs().then(() => {
          if (logContainerRef.current) {
            const newScrollHeight = logContainerRef.current.scrollHeight;
            const heightDiff = newScrollHeight - scrollHeight;
            logContainerRef.current.scrollTop = scrollTop + heightDiff;
          }
        });
      }
    };
    const id = requestAnimationFrame(() => {
      el.addEventListener('scroll', onScroll, { passive: true });
    });
    return () => {
      cancelAnimationFrame(id);
      el.removeEventListener('scroll', onScroll);
    };
  }, [isInitialLoad]);

  const tryLoadOlder = useCallback(() => {
    if (!hasMore || loading || !logContainerRef.current) return;
    const el = logContainerRef.current;
    const scrollHeight = el.scrollHeight;
    const scrollTop = el.scrollTop;
    loadMoreLogs().then(() => {
      if (logContainerRef.current) {
        const newScrollHeight = logContainerRef.current.scrollHeight;
        const heightDiff = newScrollHeight - scrollHeight;
        logContainerRef.current.scrollTop = scrollTop + heightDiff;
      }
    });
  }, [hasMore, loading, loadMoreLogs]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    const { scrollTop, scrollHeight, clientHeight } = target;
    pinnedToBottomRef.current = scrollHeight - scrollTop - clientHeight < SCROLL_BOTTOM_THRESHOLD;
    if (scrollTop < 150) tryLoadOlder();
  };

  const openInNewWindow = useCallback(async (x?: number, y?: number) => {
    await openLogsWindow({
      fallbackX: typeof x === 'number' ? x : undefined,
      fallbackY: typeof y === 'number' ? y : undefined,
    });
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
      const sx = e.screenX ?? 0;
      const sy = e.screenY ?? 0;
      const pos = (sx !== 0 || sy !== 0) ? { x: sx, y: sy } : (last ?? { x: 100, y: 100 });
      try {
        openInNewWindow(pos.x, pos.y);
      } catch (err) {
        logger.app.error('Failed to open logs window: ' + String(err), 'LogPanel');
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
    const cleanupDragOver = addGlobalEventListener(document, 'dragover', onDragOver);
    const cleanupDrop = addGlobalEventListener(document, 'drop', onDrop);
    return () => {
      cleanupDragOver();
      cleanupDrop();
    };
  }, [variant]);

  const handleClose = useCallback(() => {
    if (variant === 'embedded') {
      const { nodes, updateNode } = useLayoutStore.getState();
      const panelNode = nodes['panel'];
      updateNode('panel', {
        data: { ...panelNode?.data, visible: false },
      });
    } else {
      getCurrentWindow().close();
    }
  }, [variant]);

  const filteredLogs = useMemo(() => applyLogFilter(logs, filter), [logs, filter]);
  const safeLogs = logs ?? [];

  const handleSelectLog = useCallback((index: number) => {
    const log = filteredLogs[index] ?? null;
    setSelectedLog(log);
    if (log) {
      useEditorStore.getState().setDetailFocus({ kind: 'log' });
    } else {
      useEditorStore.getState().clearDetailFocus();
    }
  }, [filteredLogs, setSelectedLog]);

  const selectedIndex = selectedLog
    ? filteredLogs.findIndex((l) => l === selectedLog)
    : null;

  const virtualizer = useVirtualizer({
    count: filteredLogs.length,
    getScrollElement: () => logContainerRef.current,
    estimateSize: () => LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
    overscan: 8,
  });

  const getLevelColor = getLogLevelColor;
  const getLevelBgColor = getLogLevelBackground;
  const getTypeColor = getLogTypeColor;

  return (
    <div className={`flex flex-col h-full bg-[var(--workbench-bg)] text-[var(--workbench-fg)] overflow-hidden ${className}`}>
      {/* 拖拽预览图 */}
      {variant === 'embedded' &&
        createPortal(
          <div
            ref={dragImageRef}
            className="fixed -left-[9999px] -top-[9999px] w-64 rounded-lg border-2 border-[var(--accent-color)]/60 bg-[var(--workbench-bg)] overflow-hidden opacity-95 shadow-2xl pointer-events-none select-none"
            aria-hidden
          >
            <div className="flex items-center gap-2 px-3 py-2 bg-[var(--sidebar-bg)] border-b border-border">
              <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <span className="text-xs font-semibold text-foreground">{t("log.title")}</span>
              <span className="text-[10px] text-muted-foreground ml-1">{t("log.releaseToCreateWindow")}</span>
            </div>
            <div className="h-16 bg-[var(--workbench-bg)] flex items-center justify-center">
              <span className="text-xs text-muted-foreground">{t("log.dragOutsideToRelease")}</span>
            </div>
          </div>,
          document.body
        )}

      {/* 工具栏 */}
      <div
        className={`flex items-center justify-between px-3 py-2 bg-[var(--sidebar-bg)] border-b border-border shrink-0 ${variant === 'embedded' ? 'select-none cursor-grab active:cursor-grabbing' : ''}`}
        {...(variant === 'embedded' && {
          draggable: true,
          onDragStart: handleEmbeddedDragStart,
          onDragEnd: handleEmbeddedDragEnd,
          title: t('log.dragToOpenWindow'),
        })}
      >
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${loading ? 'bg-yellow-500 animate-pulse' : 'bg-green-500 animate-pulse'}`}></div>
            <span className="text-xs font-medium text-foreground">
              {loading ? t('log.loading') : t('log.monitor')}
            </span>
          </div>
          <div className="h-4 w-px bg-border"></div>
          <span className="text-xs text-muted-foreground">
            {t("log.showCount", { filtered: filteredLogs.length, total })}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1" onPointerDown={(e) => e.stopPropagation()} onMouseDown={(e) => e.stopPropagation()}>
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => refreshLogs()}
            disabled={loading}
            tooltip={t("log.refresh")}
          >
            <svg className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </ToolbarIconButton>
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => {
              const next = !autoScroll;
              setAutoScroll(next);
              if (next) {
                pinnedToBottomRef.current = true;
                const el = logContainerRef.current;
                if (el) el.scrollTop = el.scrollHeight;
              }
            }}
            className={autoScroll ? 'text-[var(--accent-color)]' : 'text-muted-foreground'}
            tooltip={autoScroll ? t('log.autoScrollEnabled') : t('log.autoScrollDisabled')}
          >
            {autoScroll ? <FiChevronDown size={14} /> : <FiChevronUp size={14} />}
          </ToolbarIconButton>
          <div className="relative">
            <ToolbarIconButton
              type="button"
              variant="ghost"
              size="icon-sm"
              ref={filterButtonRef}
              onClick={() => setIsFilterOpen(!isFilterOpen)}
              className={isFilterOpen ? 'text-[var(--accent-color)]' : 'text-muted-foreground'}
              tooltip={t("log.filter")}
            >
              <FiFilter size={14} />
            </ToolbarIconButton>
            {isFilterOpen &&
              createPortal(
                <Card
                  ref={filterPopoverRef}
                  className="fixed z-[200] w-[280px] space-y-3 p-3 shadow-2xl"
                  style={{ top: popoverPosition.top, left: popoverPosition.left }}
                  onClick={(e) => e.stopPropagation()}
                >
                <div className="relative">
                  <FiSearch className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={14} />
                  <Input
                    type="text"
                    placeholder={t("log.searchPlaceholder")}
                    value={filter?.searchText ?? ''}
                    onChange={(e) => setSearchText(e.target.value)}
                    className="pl-9"
                  />
                </div>
                <div>
                  <div className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">{t("log.level")}</div>
                  <div className="flex flex-wrap gap-2">
                    {(['error', 'warn', 'info', 'debug', 'trace'] as LogLevel[]).map((level) => (
                      <Button
                        type="button"
                        variant={filter?.levels?.has(level) ? "secondary" : "outline"}
                        size="sm"
                        key={level}
                        onClick={() => toggleLevel(level)}
                        className={filter?.levels?.has(level) ? `${getLevelBgColor(level)} ${getLevelColor(level)} border-current` : 'text-muted-foreground'}
                      >
                        {level.toUpperCase()}
                      </Button>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs font-semibold text-muted-foreground mb-2 uppercase tracking-wider">{t("log.type")}</div>
                  <div className="flex flex-wrap gap-2">
                    {(['application', 'execution', 'system', 'graph', 'data'] as LogType[]).map((type) => (
                      <Button
                        type="button"
                        variant={filter?.types?.has(type) ? "secondary" : "outline"}
                        size="sm"
                        key={type}
                        onClick={() => toggleType(type)}
                        className={filter?.types?.has(type) ? `${getTypeColor(type)} border-current` : 'text-muted-foreground'}
                      >
                        {LOG_TYPE_LABELS[type] ?? type}
                      </Button>
                    ))}
                  </div>
                </div>
              </Card>,
                document.body
              )}
          </div>
          <ToolbarIconButton
            type="button"
            variant="destructive"
            size="icon-sm"
            onClick={() => { logBuffer.clear(); setSelectedLog(null); }}
            tooltip={t("log.clear")}
          >
            <FiTrash2 size={14} />
          </ToolbarIconButton>
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={handleClose}
            tooltip={variant === 'embedded' ? t('log.closePanel') : t('log.closeWindow')}
          >
            <FiX size={14} />
          </ToolbarIconButton>
        </div>
      </div>

      {/* 日志列表 */}
      <OverlayScrollbar
        ref={logContainerRef}
        onScroll={handleScroll}
        direction="vertical"
        className="flex-1 min-h-0 bg-[var(--workbench-bg)]"
      >
        {isInitialLoad ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3">
            <div className="w-8 h-8 border-2 border-[var(--accent-color)] border-t-transparent rounded-full animate-spin"></div>
            <div className="text-sm">{t("log.loadingLogs")}</div>
          </div>
        ) : (filteredLogs?.length ?? 0) === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3">
            <svg className="w-16 h-16 opacity-30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <div className="text-center">
              <div className="text-sm font-medium mb-1">
                {safeLogs.length === 0 ? t('log.noLogs') : t('log.noMatches')}
              </div>
              <div className="text-xs opacity-60">
                {safeLogs.length === 0 ? t('log.runGraphHint') : t('log.adjustFilterHint')}
              </div>
            </div>
          </div>
        ) : (
          <>
            {loading && (
              <div className="absolute top-0 left-0 right-0 z-10 py-2 text-center text-xs text-[var(--accent-color)] flex items-center justify-center gap-2 bg-[var(--workbench-bg)]/95 pointer-events-none">
                <div className="w-3 h-3 border-2 border-[var(--accent-color)] border-t-transparent rounded-full animate-spin"></div>
                {t("log.loading")}
              </div>
            )}
            <div className="relative px-3 py-1">
              <div style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}>
                {virtualizer.getVirtualItems().map((virtualRow) => {
                  const log = filteredLogs[virtualRow.index];
                  if (!log) return null;
                  return (
                    <div
                      key={virtualRow.key}
                      data-index={virtualRow.index}
                      style={{
                        position: 'absolute',
                        top: 0,
                        left: 0,
                        width: '100%',
                        height: LOG_ITEM_HEIGHT,
                        transform: `translateY(${virtualRow.start}px)`,
                      }}
                    >
                      <LogItem
                        log={log}
                        isSelected={selectedIndex === virtualRow.index}
                        onClick={() => handleSelectLog(virtualRow.index)}
                        getLevelColor={getLevelColor}
                        getLevelBgColor={getLevelBgColor}
                        getTypeColor={getTypeColor}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          </>
        )}
      </OverlayScrollbar>

    </div>
  );
};

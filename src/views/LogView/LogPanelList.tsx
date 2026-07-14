import { useTranslation } from 'react-i18next';
import { LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from '@/app/appConfig/default';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { LogLevel, LogMessage } from '@/shared/types/ui';
import {
  getLogLevelBackground,
  getLogLevelColor,
  getLogTypeColor,
  LOG_TYPE_BACKGROUND,
  LOG_TYPE_LABELS,
} from './logPresentation';
import { useLogPanelContext } from './logPanelContext';

const LEVEL_ACCENT: Record<LogLevel, string> = {
  error: 'border-l-red-500/70',
  warn: 'border-l-amber-500/70',
  info: 'border-l-sky-500/60',
  debug: 'border-l-border',
  trace: 'border-l-border/60',
};

function LogItemRow({
  log,
  isSelected,
  onClick,
}: {
  log: LogMessage;
  isSelected: boolean;
  onClick: () => void;
}) {
  const levelColor = getLogLevelColor(log.level);
  const levelBg = getLogLevelBackground(log.level);
  const typeColor = getLogTypeColor(log.log_type);
  const typeBg = LOG_TYPE_BACKGROUND[log.log_type] ?? 'bg-muted/40';

  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        'group flex w-full items-center gap-2.5 border-b border-border/30 px-3 py-1.5 text-left transition-colors',
        'border-l-2',
        LEVEL_ACCENT[log.level] ?? 'border-l-border/50',
        isSelected
          ? 'bg-[var(--accent-color)]/8'
          : 'hover:bg-muted/30',
      ].join(' ')}
      style={{ minHeight: LOG_ITEM_HEIGHT }}
    >
      <span className="w-[52px] shrink-0 font-mono text-[10px] tabular-nums text-muted-foreground/80">
        {log.timestamp.split(' ')[1]}
      </span>
      <span
        className={`w-12 shrink-0 rounded px-1 py-0.5 text-center text-[9px] font-semibold uppercase tracking-wide ${levelBg} ${levelColor}`}
      >
        {log.level}
      </span>
      <span
        className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-medium ${typeBg} ${typeColor}`}
      >
        {LOG_TYPE_LABELS[log.log_type] ?? log.log_type.toUpperCase()}
      </span>
      {log.source ? (
        <span className="max-w-[88px] shrink-0 truncate font-mono text-[10px] text-sky-500/80">
          [{log.source}]
        </span>
      ) : null}
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] leading-5 text-foreground/90 group-hover:text-foreground">
        {log.message}
      </span>
    </button>
  );
}

export function LogPanelList() {
  const { t } = useTranslation();
  const {
    filteredLogs,
    logs,
    loading,
    isInitialLoad,
    logContainerRef,
    virtualizer,
    selectedIndex,
    handleSelectLog,
    handleScroll,
  } = useLogPanelContext();

  return (
    <OverlayScrollbar
      ref={logContainerRef}
      onScroll={handleScroll}
      direction="vertical"
      className="relative min-h-0 flex-1 bg-[var(--workbench-bg)]"
    >
      {isInitialLoad ? (
        <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--accent-color)] border-t-transparent" />
          <p className="text-xs">{t('log.loadingLogs')}</p>
        </div>
      ) : filteredLogs.length === 0 ? (
        <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center text-muted-foreground">
          <svg className="h-10 w-10 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden>
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.25}
              d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
            />
          </svg>
          <p className="text-sm font-medium text-foreground/70">
            {logs.length === 0 ? t('log.noLogs') : t('log.noMatches')}
          </p>
          <p className="max-w-xs text-[11px] text-muted-foreground/80">
            {logs.length === 0 ? t('log.runGraphHint') : t('log.adjustFilterHint')}
          </p>
        </div>
      ) : (
        <>
          {loading ? (
            <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-center justify-center gap-2 border-b border-border/40 bg-[var(--workbench-bg)]/90 py-1.5 text-[10px] text-[var(--accent-color)]">
              <div className="h-2.5 w-2.5 animate-spin rounded-full border border-[var(--accent-color)] border-t-transparent" />
              {t('log.loading')}
            </div>
          ) : null}
          <div className="py-0.5">
            <div
              style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}
            >
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
                      height: LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    <LogItemRow
                      log={log}
                      isSelected={selectedIndex === virtualRow.index}
                      onClick={() => handleSelectLog(virtualRow.index)}
                    />
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </OverlayScrollbar>
  );
}

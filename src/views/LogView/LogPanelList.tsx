import { useTranslation } from 'react-i18next';
import { LogPanelVirtualList } from './LogPanelVirtualList';
import { useLogPanelContext } from './logPanelContext';

export function LogPanelList() {
  const { t } = useTranslation();
  const {
    filteredLogs,
    logs,
    loading,
    hasMore,
    loadMoreLogs,
    isInitialLoad,
    activeLogTypeTab,
    autoScroll,
    refreshScrollToken,
    variant,
    selectedIndex,
    handleSelectLog,
  } = useLogPanelContext();

  if (isInitialLoad) {
    return (
      <div className="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-[var(--workbench-bg)] text-muted-foreground">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--accent-color)] border-t-transparent" />
        <p className="text-xs">{t('log.loadingLogs')}</p>
      </div>
    );
  }

  if (filteredLogs.length === 0) {
    return (
      <div className="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-2 bg-[var(--workbench-bg)] px-6 text-center text-muted-foreground">
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
    );
  }

  return (
    <LogPanelVirtualList
      key={activeLogTypeTab}
      logs={filteredLogs}
      loading={loading}
      hasMore={hasMore}
      loadMoreLogs={loadMoreLogs}
      autoScroll={autoScroll}
      refreshScrollToken={refreshScrollToken}
      variant={variant}
      selectedIndex={selectedIndex}
      onSelectLog={handleSelectLog}
    />
  );
}

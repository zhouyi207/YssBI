import { useTranslation } from 'react-i18next';
import { VscFile } from 'react-icons/vsc';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
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
      <Empty className="relative min-h-0 rounded-none bg-[var(--workbench-bg)] px-6">
        <EmptyHeader>
          <EmptyMedia variant="icon" className="text-muted-foreground">
            <VscFile />
          </EmptyMedia>
          <EmptyTitle>{logs.length === 0 ? t('log.noLogs') : t('log.noMatches')}</EmptyTitle>
          <EmptyDescription>
            {logs.length === 0 ? t('log.runGraphHint') : t('log.adjustFilterHint')}
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
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

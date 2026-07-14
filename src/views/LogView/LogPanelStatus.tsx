import { useTranslation } from 'react-i18next';
import { useLogPanelContext } from './logPanelContext';

export function LogPanelStatus() {
  const { t } = useTranslation();
  const { loading, filteredLogs, total } = useLogPanelContext();

  return (
    <div className="flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
      <span
        className={`h-1.5 w-1.5 shrink-0 rounded-full ${loading ? 'bg-amber-400 animate-pulse' : 'bg-emerald-500/80'}`}
        aria-hidden
      />
      <span className="truncate">
        {t('log.showCount', { filtered: filteredLogs.length, total })}
      </span>
    </div>
  );
}

import { useTranslation } from 'react-i18next';
import { useLogPanelContext } from './logPanelContext';

const STATUS_COLOR = {
  connecting: 'bg-amber-400 animate-pulse',
  live: 'bg-emerald-500/80',
  error: 'bg-red-400',
} as const;

export function LogPanelStatus() {
  const { t } = useTranslation();
  const { filteredLogs, total, subscriptionStatus } = useLogPanelContext();

  return (
    <div className="flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
      <span
        className={`h-1.5 w-1.5 shrink-0 rounded-full ${STATUS_COLOR[subscriptionStatus]}`}
        aria-hidden
      />
      <span className="truncate">
        {t('log.showCount', { filtered: filteredLogs.length, total })}
      </span>
    </div>
  );
}

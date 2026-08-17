import { useTranslation } from 'react-i18next';
import { LOG_DOMAIN_TAB_ORDER, type LogDomainTabId } from '@/features/core/log/logDomainTabs';
import { useLogStore } from '@/features/core/log/logStore';
import { cn } from '@/lib/utils';
import { LOG_DOMAIN_LABELS } from './logPresentation';

function getTabLabel(tab: LogDomainTabId, t: (key: string) => string): string {
  if (tab === 'all') return t('log.typeAll');
  return LOG_DOMAIN_LABELS[tab] ?? tab;
}

export function LogDomainTabStrip() {
  const { t } = useTranslation();
  const activeTab = useLogStore((state) => state.activeLogDomainTab);
  const setActiveTab = useLogStore((state) => state.setActiveLogDomainTab);

  return (
    <div className="flex h-full shrink-0 items-end gap-0">
      {LOG_DOMAIN_TAB_ORDER.map((tab) => {
        const active = tab === activeTab;
        return (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            className={cn(
              'relative px-3 pb-1.5 pt-1 text-[11px] font-medium uppercase tracking-wide transition-colors',
              active
                ? 'text-foreground after:absolute after:inset-x-1 after:bottom-0 after:h-0.5 after:bg-[var(--accent-color)]'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            {getTabLabel(tab, t)}
          </button>
        );
      })}
    </div>
  );
}

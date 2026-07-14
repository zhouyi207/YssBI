import { useTranslation } from 'react-i18next';
import { useLogStore } from '@/features/core/log/logStore';
import { cn } from '@/lib/utils';
import { LOG_TYPE_LABELS } from './logPresentation';
import { LOG_TYPE_TAB_ORDER, type LogTypeTabId } from '@/features/core/log/logTypeTabs';

function getTabLabel(tab: LogTypeTabId, t: (key: string) => string): string {
  if (tab === 'all') return t('log.typeAll');
  return LOG_TYPE_LABELS[tab] ?? tab;
}

export function LogTypeTabStrip() {
  const { t } = useTranslation();
  const activeLogTypeTab = useLogStore((s) => s.activeLogTypeTab);
  const setActiveLogTypeTab = useLogStore((s) => s.setActiveLogTypeTab);

  return (
    <div className="flex h-full shrink-0 items-end gap-0">
      {LOG_TYPE_TAB_ORDER.map((tab) => {
        const active = tab === activeLogTypeTab;
        return (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveLogTypeTab(tab)}
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

import { useTranslation } from 'react-i18next';
import { useEditorHistoryAvailability } from '@/features/application/editor';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';

export function SidebarCommandsTab() {
  const { t } = useTranslation();
  const { activeTabId, canUndo, canRedo, pending } = useEditorHistoryAvailability();

  if (!activeTabId) {
    return (
      <SidebarTabPanel>
        <div className="p-3 text-xs text-muted-foreground">{t('sidebar.noActiveGraph')}</div>
      </SidebarTabPanel>
    );
  }

  return (
    <SidebarTabPanel>
      <div className="flex flex-col gap-1 p-2 text-xs text-muted-foreground">
        <div className="flex h-7 items-center justify-between rounded-sm px-2">
          <span>{t('common.undo')}</span>
          <span aria-label={canUndo ? 'available' : 'unavailable'}>{pending ? '…' : canUndo ? '✓' : '—'}</span>
        </div>
        <div className="flex h-7 items-center justify-between rounded-sm px-2">
          <span>{t('common.redo')}</span>
          <span aria-label={canRedo ? 'available' : 'unavailable'}>{pending ? '…' : canRedo ? '✓' : '—'}</span>
        </div>
      </div>
    </SidebarTabPanel>
  );
}

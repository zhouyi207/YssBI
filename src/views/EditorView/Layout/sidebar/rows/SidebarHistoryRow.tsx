import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { HistoryEntry } from '@/features/core/history';
import { SidebarListItem, sidebarTrailingMetaClass } from '../../sidebarUi';

function formatTime(ts: number): string {
  const d = new Date(ts);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`;
}

export const SidebarHistoryRow = memo(function SidebarHistoryRow({
  entry,
  icon,
  isHighlighted = false,
  indentDepth = 0,
}: {
  entry: HistoryEntry;
  icon: React.ReactNode;
  isHighlighted?: boolean;
  indentDepth?: number;
}) {
  const { t } = useTranslation();

  return (
    <SidebarListItem
      id={entry.id}
      isSelected={isHighlighted}
      indentDepth={indentDepth}
      icon={icon}
      label={t(`sidebar.commands.${entry.commandType}`, { defaultValue: entry.commandType })}
      trailing={
        <span className={sidebarTrailingMetaClass()}>{formatTime(entry.timestamp)}</span>
      }
    />
  );
});

import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscGraphLine } from 'react-icons/vsc';
import { TYPE_ICON_COLORS } from '@/features/domain/sidebar';
import { SidebarListItem, SidebarRowActionButton, SIDEBAR_ROW_ICON_SIZE } from '../../sidebarUi';

export const SidebarWorksheetRow = memo(function SidebarWorksheetRow({
  id,
  name,
  indentDepth = 0,
  isSelected = false,
  onOpen,
  onContextMenu,
}: {
  id: string;
  name: string;
  indentDepth?: number;
  isSelected?: boolean;
  onOpen: (id: string, name: string) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();

  return (
    <SidebarListItem
      id={id}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscGraphLine size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.worksheet }} />}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        void onOpen(id, name);
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        void onOpen(id, name);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <SidebarRowActionButton
          isSelected={isSelected}
          tooltip={t('sidebar.open')}
          onClick={(e) => {
            e.stopPropagation();
            void onOpen(id, name);
          }}
        />
      }
    />
  );
});

import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscGraphLine } from 'react-icons/vsc';
import { TYPE_ICON_COLORS } from '@/features/application/viewCapabilities';
import { SidebarListItem, SidebarRowActionButton, SIDEBAR_ROW_ICON_SIZE } from '../../sidebarUi';

export const SidebarWorksheetRow = memo(function SidebarWorksheetRow({
  worksheetPath,
  name,
  indentDepth = 0,
  isSelected = false,
  onOpen,
  onContextMenu,
}: {
  worksheetPath: string;
  name: string;
  indentDepth?: number;
  isSelected?: boolean;
  onOpen: (worksheetPath: string, name: string) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();

  return (
    <SidebarListItem
      id={worksheetPath}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscGraphLine size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.worksheet }} />}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        void onOpen(worksheetPath, name);
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        void onOpen(worksheetPath, name);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <SidebarRowActionButton
          isSelected={isSelected}
          tooltip={t('sidebar.open')}
          onClick={(e) => {
            e.stopPropagation();
            void onOpen(worksheetPath, name);
          }}
        />
      }
    />
  );
});

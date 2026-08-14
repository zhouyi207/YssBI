import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscDebugDisconnect, VscTrash } from 'react-icons/vsc';
import {
  ContextMenu,
  type ContextMenuPosition,
  type ContextMenuSection,
} from '@/shared/ui/contextMenu';

export interface ConnectionContextMenuProps {
  position: ContextMenuPosition;
  selectedCount: number;
  onBreak: () => void;
  onClose: () => void;
}

export const ConnectionContextMenu: React.FC<ConnectionContextMenuProps> = ({
  position,
  selectedCount,
  onBreak,
  onClose,
}) => {
  const { t } = useTranslation();
  const sections = useMemo((): ContextMenuSection[] => [{
    items: [
      {
        id: 'break',
        label: t(selectedCount > 1
          ? 'contextMenu.connection.breakSelectedLinks'
          : 'contextMenu.connection.breakLink'),
        icon: <VscDebugDisconnect size={12} />,
        onClick: onBreak,
      },
      {
        id: 'delete',
        label: t('contextMenu.connection.delete'),
        icon: <VscTrash size={12} />,
        shortcut: 'Del',
        danger: true,
        onClick: onBreak,
      },
    ],
  }], [onBreak, selectedCount, t]);

  return <ContextMenu position={position} sections={sections} onClose={onClose} />;
};

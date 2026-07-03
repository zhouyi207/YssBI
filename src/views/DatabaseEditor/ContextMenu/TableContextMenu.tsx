import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ContextMenu, type ContextMenuSection } from '@/shared/ui/contextMenu';

export interface ContextMenuState {
  x: number;
  y: number;
  type: 'cell' | 'header' | 'row';
  rowIndex?: number;
  colIndex?: number;
  colName?: string;
}

interface TableContextMenuProps {
  menu: ContextMenuState;
  selectedRowIndices: number[];
  onAddRow: (index?: number) => void;
  onDeleteRow: (indices: number[]) => void;
  onRenameColumn: (name: string) => void;
  onAddColumn: () => void;
  onDeleteColumn: (name: string) => void;
  onClearSelection: () => void;
  onClose: () => void;
}

export const TableContextMenu: React.FC<TableContextMenuProps> = ({
  menu,
  selectedRowIndices,
  onAddRow,
  onDeleteRow,
  onRenameColumn,
  onAddColumn,
  onDeleteColumn,
  onClearSelection,
  onClose,
}) => {
  const { t } = useTranslation();

  const multiRow =
    selectedRowIndices.length > 1
    && menu.rowIndex !== undefined
    && selectedRowIndices.includes(menu.rowIndex);

  const sections = useMemo((): ContextMenuSection[] => {
    if ((menu.type === 'cell' || menu.type === 'row') && menu.rowIndex !== undefined) {
      const r = menu.rowIndex;
      return [
        {
          items: [
            {
              id: 'insert-above',
              label: t('databaseEditor.insertRowAbove'),
              onClick: () => {
                void onAddRow(r);
              },
            },
            {
              id: 'insert-below',
              label: t('databaseEditor.insertRowBelow'),
              onClick: () => {
                void onAddRow(r + 1);
              },
            },
          ],
        },
        {
          items: [
            multiRow
              ? {
                  id: 'delete-rows',
                  label: t('databaseEditor.deleteRows', { count: selectedRowIndices.length }),
                  danger: true,
                  onClick: () => {
                    void onDeleteRow(selectedRowIndices);
                    onClearSelection();
                  },
                }
              : {
                  id: 'delete-row',
                  label: t('databaseEditor.deleteRow'),
                  danger: true,
                  onClick: () => {
                    void onDeleteRow([r]);
                  },
                },
          ],
        },
      ];
    }

    if (menu.type === 'header' && menu.colName) {
      const name = menu.colName;
      return [
        {
          items: [
            {
              id: 'rename',
              label: t('databaseEditor.renameColumn'),
              onClick: () => {
                void onRenameColumn(name);
              },
            },
            {
              id: 'add-column',
              label: t('databaseEditor.addColumn'),
              onClick: () => {
                void onAddColumn();
              },
            },
          ],
        },
        {
          items: [
            {
              id: 'delete-column',
              label: t('databaseEditor.deleteColumn', { name }),
              danger: true,
              onClick: () => {
                void onDeleteColumn(name);
              },
            },
          ],
        },
      ];
    }

    return [];
  }, [
    menu.type,
    menu.rowIndex,
    menu.colName,
    multiRow,
    selectedRowIndices,
    t,
    onAddRow,
    onDeleteRow,
    onRenameColumn,
    onAddColumn,
    onDeleteColumn,
    onClearSelection,
  ]);

  if (sections.length === 0) {
    return null;
  }

  return (
    <ContextMenu
      position={{ x: menu.x, y: menu.y }}
      sections={sections}
      onClose={onClose}
    />
  );
};

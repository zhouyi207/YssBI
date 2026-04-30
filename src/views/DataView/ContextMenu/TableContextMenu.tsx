import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';

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
  menu, selectedRowIndices, onAddRow, onDeleteRow,
  onRenameColumn, onAddColumn, onDeleteColumn, onClearSelection, onClose,
}) => {
  const { t } = useTranslation();
  const multiRow = selectedRowIndices.length > 1 && menu.rowIndex !== undefined && selectedRowIndices.includes(menu.rowIndex);

  return (
    <Card
      className="fixed z-[100] min-w-[160px] py-1 select-none shadow-xl"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      {menu.type === 'cell' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            {t("dataView.insertRowAbove")}
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            {t("dataView.insertRowBelow")}
          </Button>
          {multiRow ? (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              {t("dataView.deleteRows", { count: selectedRowIndices.length })}
            </Button>
          ) : (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              {t("dataView.deleteRow")}
            </Button>
          )}
        </>
      )}
      {menu.type === 'row' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            {t("dataView.insertRowAbove")}
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            {t("dataView.insertRowBelow")}
          </Button>
          {multiRow ? (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              {t("dataView.deleteRows", { count: selectedRowIndices.length })}
            </Button>
          ) : (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              {t("dataView.deleteRow")}
            </Button>
          )}
        </>
      )}
      {menu.type === 'header' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onRenameColumn(menu.colName!); onClose(); }}>
            {t("dataView.renameColumn")}
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddColumn(); onClose(); }}>
            {t("dataView.addColumn")}
          </Button>
          <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteColumn(menu.colName!); onClose(); }}>
            {t("dataView.deleteColumn", { name: menu.colName })}
          </Button>
        </>
      )}
    </Card>
  );
};

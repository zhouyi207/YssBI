import React from 'react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';

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
  onStartEdit: (row: number, col: number) => void;
  onAddRow: (index?: number) => void;
  onDeleteRow: (indices: number[]) => void;
  onRenameColumn: (name: string) => void;
  onAddColumn: () => void;
  onDeleteColumn: (name: string) => void;
  onClearSelection: () => void;
  onClose: () => void;
}

export const TableContextMenu: React.FC<TableContextMenuProps> = ({
  menu, selectedRowIndices, onStartEdit, onAddRow, onDeleteRow,
  onRenameColumn, onAddColumn, onDeleteColumn, onClearSelection, onClose,
}) => {
  const multiRow = selectedRowIndices.length > 1 && menu.rowIndex !== undefined && selectedRowIndices.includes(menu.rowIndex);

  return (
    <Card
      className="fixed z-[100] min-w-[160px] py-1 select-none shadow-xl"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      {menu.type === 'cell' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onStartEdit(menu.rowIndex!, menu.colIndex!); onClose(); }}>
            Edit Cell
          </Button>
          <Separator className="my-1" />
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            Insert Row Above
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            Insert Row Below
          </Button>
          {multiRow ? (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              Delete {selectedRowIndices.length} Rows
            </Button>
          ) : (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              Delete Row
            </Button>
          )}
        </>
      )}
      {menu.type === 'row' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            Insert Row Above
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            Insert Row Below
          </Button>
          {multiRow ? (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              Delete {selectedRowIndices.length} Rows
            </Button>
          ) : (
            <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              Delete Row
            </Button>
          )}
        </>
      )}
      {menu.type === 'header' && (
        <>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onRenameColumn(menu.colName!); onClose(); }}>
            Rename Column
          </Button>
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onAddColumn(); onClose(); }}>
            Add Column
          </Button>
          <Button type="button" variant="destructive" size="sm" className="h-auto w-full justify-start rounded-none px-3 py-1.5 text-[11px]" onClick={() => { onDeleteColumn(menu.colName!); onClose(); }}>
            Delete Column &quot;{menu.colName}&quot;
          </Button>
        </>
      )}
    </Card>
  );
};

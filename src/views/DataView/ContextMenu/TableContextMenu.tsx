import React from 'react';

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

const btnClass = "w-full text-left px-3 py-1.5 text-[11px] text-gray-300 hover:bg-[var(--accent-color)]/20 hover:text-white transition-colors";
const dangerClass = btnClass + ' text-red-400 hover:text-red-300';

export const TableContextMenu: React.FC<TableContextMenuProps> = ({
  menu, selectedRowIndices, onStartEdit, onAddRow, onDeleteRow,
  onRenameColumn, onAddColumn, onDeleteColumn, onClearSelection, onClose,
}) => {
  const multiRow = selectedRowIndices.length > 1 && menu.rowIndex !== undefined && selectedRowIndices.includes(menu.rowIndex);

  return (
    <div
      className="fixed z-[100] min-w-[160px] bg-[var(--sidebar-bg)] border border-gray-700 rounded shadow-xl py-1 select-none"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      {menu.type === 'cell' && (
        <>
          <button className={btnClass} onClick={() => { onStartEdit(menu.rowIndex!, menu.colIndex!); onClose(); }}>
            Edit Cell
          </button>
          <div className="h-px bg-gray-700 my-1" />
          <button className={btnClass} onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            Insert Row Above
          </button>
          <button className={btnClass} onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            Insert Row Below
          </button>
          {multiRow ? (
            <button className={dangerClass} onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              Delete {selectedRowIndices.length} Rows
            </button>
          ) : (
            <button className={dangerClass} onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              Delete Row
            </button>
          )}
        </>
      )}
      {menu.type === 'row' && (
        <>
          <button className={btnClass} onClick={() => { onAddRow(menu.rowIndex!); onClose(); }}>
            Insert Row Above
          </button>
          <button className={btnClass} onClick={() => { onAddRow(menu.rowIndex! + 1); onClose(); }}>
            Insert Row Below
          </button>
          {multiRow ? (
            <button className={dangerClass} onClick={() => { onDeleteRow(selectedRowIndices); onClearSelection(); onClose(); }}>
              Delete {selectedRowIndices.length} Rows
            </button>
          ) : (
            <button className={dangerClass} onClick={() => { onDeleteRow([menu.rowIndex!]); onClose(); }}>
              Delete Row
            </button>
          )}
        </>
      )}
      {menu.type === 'header' && (
        <>
          <button className={btnClass} onClick={() => { onRenameColumn(menu.colName!); onClose(); }}>
            Rename Column
          </button>
          <button className={btnClass} onClick={() => { onAddColumn(); onClose(); }}>
            Add Column
          </button>
          <button className={dangerClass} onClick={() => { onDeleteColumn(menu.colName!); onClose(); }}>
            Delete Column &quot;{menu.colName}&quot;
          </button>
        </>
      )}
    </div>
  );
};

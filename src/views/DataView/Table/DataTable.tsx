import React, { useRef, useState, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { VscDatabase } from 'react-icons/vsc';
import type { ColumnMeta, CellPos, SelectionRange } from '@/features/application/dataView';
import { selectionBounds, COLUMN_TYPE_OPTIONS } from '@/features/application/dataView';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { Select } from '@/shared/ui';
import { DATA_VIEW_ROW_HEIGHT, DATA_VIEW_ROW_NUM_WIDTH, DATA_VIEW_MIN_COLUMNS } from '@/app/appConfig/default';

interface ContextMenuTarget {
  type: 'cell' | 'header' | 'row';
  rowIndex?: number;
  colIndex?: number;
  colName?: string;
}

interface DataTableProps {
  columns: ColumnMeta[];
  loadedRows: any[][];
  totalRowCount: number;
  loading: boolean;
  loadingMore: boolean;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onHeaderHeightChange: (h: number) => void;

  // selection
  selection: SelectionRange | null;
  activeCell: CellPos | null;
  editingCell: { row: number; col: number } | null;
  isInSelection: (row: number, col: number) => boolean;
  onCellMouseDown: (row: number, col: number, e: React.MouseEvent) => void;
  onCellMouseEnter: (row: number, col: number) => void;
  onRowHeaderClick: (row: number, e: React.MouseEvent) => void;
  onColHeaderClick: (col: number, e: React.MouseEvent) => void;
  onSelectAll: () => void;

  // editing
  editValue: string;
  editInputRef: React.RefObject<HTMLInputElement | null>;
  onEditValueChange: (v: string) => void;
  onStartEdit: (row: number, col: number) => void;
  onCommitEdit: () => Promise<void>;
  onCancelEdit: () => void;

  // context menu
  onContextMenu: (e: React.MouseEvent, target: ContextMenuTarget) => void;

  // cast column type
  onCastColumn?: (colName: string, newDtype: string) => void;

  // scroll
  onScroll: (e: React.UIEvent<HTMLDivElement>) => void;
}

export const DataTable: React.FC<DataTableProps> = ({
  columns, loadedRows, totalRowCount, loading, loadingMore, scrollRef, onHeaderHeightChange,
  selection, activeCell, editingCell, isInSelection,
  onCellMouseDown, onCellMouseEnter, onRowHeaderClick, onColHeaderClick, onSelectAll,
  editValue, editInputRef, onEditValueChange, onStartEdit, onCommitEdit, onCancelEdit,
  onContextMenu, onCastColumn, onScroll,
}) => {
  const headerRef = useRef<HTMLTableSectionElement>(null);
  const [headerHeight, setHeaderHeight] = useState(0);
  const [hoveredCell, setHoveredCell] = useState<{ row: number; col: number } | null>(null);

  useEffect(() => {
    const el = headerRef.current;
    if (!el) { setHeaderHeight(0); onHeaderHeightChange(0); return; }
    const update = () => { setHeaderHeight(el.offsetHeight); onHeaderHeightChange(el.offsetHeight); };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [columns.length, onHeaderHeightChange]);

  /** 固定总高度：有 totalRowCount 时用其作为虚拟化数量，避免懒加载时高度变化 */
  const virtualRowCount = totalRowCount > 0 ? totalRowCount : loadedRows.length;
  const rowVirtualizer = useVirtualizer({
    count: virtualRowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => DATA_VIEW_ROW_HEIGHT,
    overscan: 20,
  });

  const virtualRows = rowVirtualizer.getVirtualItems();
  const totalTableHeight = virtualRowCount * DATA_VIEW_ROW_HEIGHT;
  const hasData = columns.length > 0;
  /** 显示列数：至少 20 列，不足时用空列填充 */
  const displayColumnCount = Math.max(columns.length, DATA_VIEW_MIN_COLUMNS);

  if (!hasData) {
    return (
      <div className="flex-1 min-h-0 min-w-0 flex flex-col items-center justify-center gap-4 bg-[var(--workbench-bg)]">
        <VscDatabase className="text-gray-500/60" size={48} />
        <span className="text-sm font-medium tracking-widest uppercase text-gray-500/70">
          {loading ? 'Loading project data...' : 'No DataFrame Selected'}
        </span>
      </div>
    );
  }

  const colSpanForSpacer = displayColumnCount + 1;

  return (
    <div className="relative flex-1 min-h-0 min-w-0 overflow-hidden flex flex-col">
      <OverlayScrollbar
        ref={scrollRef}
        onScroll={onScroll}
        direction="both"
        className="flex-1 min-h-0 min-w-0 bg-[var(--workbench-bg)]"
        scrollbarOffsetTop={headerHeight}
        scrollbarOffsetLeft={DATA_VIEW_ROW_NUM_WIDTH}
      >
        <div
          className="min-w-full inline-block align-middle"
          style={{ minHeight: totalTableHeight }}
          onMouseLeave={() => setHoveredCell(null)}
        >
          <table className="border-collapse min-w-full w-max" style={{ tableLayout: 'auto' }}>
            <thead ref={headerRef} className="sticky top-0 z-10 bg-[var(--sidebar-bg)] border-b border-gray-700">
              <tr>
                <th
                  className="p-2 text-left text-[10px] font-black uppercase text-gray-500 border-r border-gray-800 text-center shrink-0 invisible"
                  style={{ width: DATA_VIEW_ROW_NUM_WIDTH, minWidth: DATA_VIEW_ROW_NUM_WIDTH }}
                  aria-hidden
                >#</th>
              {Array.from({ length: displayColumnCount }, (_, i) => {
                const col = columns[i];
                const isPlaceholder = !col;
                const colSelected = !isPlaceholder && selection && (() => {
                  const { c0, c1 } = selectionBounds(selection);
                  return i >= c0 && i <= c1;
                })();
                const colHovered = hoveredCell?.col === i;
                return (
                  <th
                    key={i}
                    className={`p-2 text-left border-r border-gray-800 group cursor-default min-w-[80px] ${colSelected ? 'bg-[var(--accent-color)]/10' : ''} ${colHovered && !colSelected ? 'bg-white/[0.02]' : ''}`}
                    onClick={isPlaceholder ? undefined : (e) => onColHeaderClick(i, e)}
                    onContextMenu={isPlaceholder ? undefined : (e) => onContextMenu(e, { type: 'header', colIndex: i, colName: col!.name })}
                    onMouseEnter={isPlaceholder ? undefined : () => setHoveredCell({ row: -1, col: i })}
                  >
                    {isPlaceholder ? (
                      <span className="text-[11px] text-gray-600/50">&nbsp;</span>
                    ) : (
                      <div className="grid grid-cols-[minmax(0,1fr)_80px] gap-2 items-center min-w-0">
                        <span className="text-[11px] font-bold text-gray-300 truncate min-w-0">{col.name}</span>
                        {onCastColumn ? (
                          <div className="w-full min-w-0" onClick={(e) => e.stopPropagation()}>
                            <Select
                              value={col.type}
                              onChange={(v) => { if (v !== col.type) onCastColumn(col.name, v); }}
                              options={(() => {
                                const opts = COLUMN_TYPE_OPTIONS.map(o => ({ label: o.label, value: o.value }));
                                if (col.type && !opts.some(o => o.value === col.type)) opts.unshift({ label: col.type, value: col.type });
                                return opts;
                              })()}
                              className="text-[9px] h-5 font-mono !w-full"
                            />
                          </div>
                        ) : (
                          <span className="text-[9px] text-[var(--accent-color)]/60 font-mono shrink-0">{col.type}</span>
                        )}
                      </div>
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {virtualRows.length > 0 && virtualRows[0].start > 0 && (
              <tr aria-hidden>
                <td colSpan={colSpanForSpacer} style={{ height: virtualRows[0].start, padding: 0, border: 'none' }} />
              </tr>
            )}
            {virtualRows.map((virtualRow) => {
              const row = loadedRows[virtualRow.index];
              const hasData = row !== undefined;
              const ri = virtualRow.index;
              const rowData = hasData && Array.isArray(row) ? row : [];
              const rowSelected = selection && (() => {
                const { r0, r1 } = selectionBounds(selection);
                return ri >= r0 && ri <= r1;
              })();
              const rowActive = activeCell?.row === ri;
              const rowHovered = hoveredCell !== null && hoveredCell.row === ri;
              return (
                <tr
                  key={virtualRow.key}
                  data-index={ri}
                  className="group transition-colors hover:bg-white/[0.02]"
                  style={{ height: DATA_VIEW_ROW_HEIGHT }}
                >
                  <td
                    className={`p-2 text-[11px] font-bold text-gray-300 border-r border-gray-800 text-center cursor-default select-none sticky left-0 z-10 shrink-0 bg-[var(--sidebar-bg)] ${rowHovered && !(rowSelected || rowActive) ? '!bg-white/[0.02]' : ''} ${!rowHovered && !(rowSelected || rowActive) ? 'group-hover:!bg-white/[0.02]' : ''}`}
                    style={{
                      width: DATA_VIEW_ROW_NUM_WIDTH,
                      minWidth: DATA_VIEW_ROW_NUM_WIDTH,
                      ...((rowSelected || rowActive) ? { background: 'color-mix(in srgb, var(--accent-color) 10%, var(--sidebar-bg))' } : {}),
                    }}
                    onClick={hasData ? (e) => onRowHeaderClick(ri, e) : undefined}
                    onContextMenu={hasData ? (e) => onContextMenu(e, { type: 'row', rowIndex: ri }) : undefined}
                    onMouseEnter={() => setHoveredCell({ row: ri, col: -1 })}
                  >
                    {ri + 1}
                  </td>
                  {Array.from({ length: displayColumnCount }, (_, j) => {
                    const isPlaceholder = j >= columns.length;
                    const val = rowData[j];
                    const isEditingThis = editingCell?.row === ri && editingCell?.col === j;
                    const isSel = !isPlaceholder && isInSelection(ri, j);
                    const isActive = activeCell?.row === ri && activeCell?.col === j;
                    const colHovered = hoveredCell !== null && hoveredCell.col === j;
                    return (
                      <td
                        key={j}
                        className={`p-0 text-[11px] text-gray-400 border-r border-gray-800/50 min-w-[80px] ${isSel ? 'bg-[var(--accent-color)]/8' : ''} ${isActive && !isEditingThis ? 'ring-1 ring-inset ring-[var(--accent-color)]/60' : ''} ${colHovered && !isSel ? 'bg-white/[0.02]' : ''}`}
                        onMouseDown={isPlaceholder || !hasData ? undefined : (e) => onCellMouseDown(ri, j, e)}
                        onMouseEnter={isPlaceholder || !hasData ? undefined : () => { setHoveredCell({ row: ri, col: j }); onCellMouseEnter(ri, j); }}
                        onDoubleClick={isPlaceholder || !hasData ? undefined : () => onStartEdit(ri, j)}
                        onContextMenu={isPlaceholder || !hasData ? undefined : (e) => onContextMenu(e, { type: 'cell', rowIndex: ri, colIndex: j, colName: columns[j]?.name })}
                      >
                        {isPlaceholder ? (
                          <div className="px-2 py-1.5 cursor-default select-none">&nbsp;</div>
                        ) : !hasData ? (
                          <div className="px-2 py-1.5 truncate cursor-default select-none text-gray-600">…</div>
                        ) : isEditingThis ? (
                          <input
                            ref={editInputRef}
                            value={editValue}
                            onChange={(e) => onEditValueChange(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') { e.preventDefault(); onCommitEdit(); }
                              else if (e.key === 'Escape') onCancelEdit();
                              else if (e.key === 'Tab') {
                                e.preventDefault();
                                onCommitEdit().then(() => {
                                  const nextCol = e.shiftKey ? j - 1 : j + 1;
                                  if (nextCol >= 0 && nextCol < columns.length) onStartEdit(ri, nextCol);
                                });
                              }
                            }}
                            onBlur={onCommitEdit}
                            className="w-full h-full px-2 py-1.5 bg-[var(--accent-color)]/10 text-gray-200 text-[11px] outline-none border border-[var(--accent-color)]/40 font-mono"
                            autoFocus
                          />
                        ) : (
                          <div className="px-2 py-1.5 truncate cursor-default select-none">
                            {val === null ? <span className="italic opacity-30">null</span> : String(val)}
                          </div>
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
            {virtualRows.length > 0 && (
              <tr aria-hidden>
                <td
                  colSpan={colSpanForSpacer}
                  style={{ height: rowVirtualizer.getTotalSize() - (virtualRows[virtualRows.length - 1]?.end ?? 0), padding: 0, border: 'none' }}
                />
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </OverlayScrollbar>
    {/* 固定在左上角的独立角格，不随滚动移动 */}
    <div
      className="absolute top-0 left-0 z-30 flex items-center justify-center cursor-pointer hover:bg-white/[0.02] bg-[var(--sidebar-bg)] border-r border-b border-gray-800 text-[11px] font-bold text-gray-300"
      style={{ width: DATA_VIEW_ROW_NUM_WIDTH, height: headerHeight || 40 }}
      onClick={onSelectAll}
    >
      #
    </div>
  </div>
  );
};

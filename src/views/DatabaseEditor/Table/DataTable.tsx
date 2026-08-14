import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DataEditor,
  GridCellKind,
  type EditableGridCell,
  type GridCell,
  type GridColumn,
  type GridSelection,
  type Item,
} from '@glideapps/glide-data-grid';
import '@glideapps/glide-data-grid/dist/index.css';
import { VscDatabase } from 'react-icons/vsc';
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import type { ColumnInfo, DatabaseRow } from '@/shared/types/dto/database';
import { emptyGridSelection, isEmptyGridSelection } from '@/features/application/databaseEditor';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { buildDataGridThemeOverlay, buildRowMarkerThemeOverlay } from './dataGridTheme';
import {
  DATABASE_EDITOR_ROW_HEIGHT,
  DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
  DATABASE_EDITOR_MIN_COLUMNS,
} from '@/app/appConfig/default';

interface ContextMenuTarget {
  type: 'cell' | 'header' | 'row';
  rowIndex?: number;
  colIndex?: number;
  colName?: string;
}

interface DataTableProps {
  columns: ColumnInfo[];
  loadedRows: DatabaseRow[];
  pageStartIndex: number;
  loading: boolean;

  // selection：与 Glide `GridSelection` 一致（受控），无独立领域模型
  selection: GridSelection | null;
  onSelectionChange: (selection: GridSelection | null) => void;

  onCommitCellValue: (row: number, col: number, value: unknown) => Promise<void>;

  // context menu
  onContextMenu: (position: { x: number; y: number }, target: ContextMenuTarget) => void;
}

function cellToValue(cell: EditableGridCell): unknown {
  switch (cell.kind) {
    case GridCellKind.Number:
      return cell.data;
    case GridCellKind.Boolean:
      return cell.data;
    case GridCellKind.Text:
    case GridCellKind.Markdown:
    case GridCellKind.Uri:
      return cell.data;
    default:
      return cell.copyData ?? '';
  }
}

export const DataTable: React.FC<DataTableProps> = ({
  columns, loadedRows, pageStartIndex, loading,
  selection,
  onSelectionChange,
  onCommitCellValue,
  onContextMenu,
}) => {
  const { t } = useTranslation();
  const appTheme = useSettingsStore((s) => s.theme);
  const dataGridTheme = useMemo(() => buildDataGridThemeOverlay(appTheme), [appTheme]);
  const rowMarkerTheme = useMemo(() => buildRowMarkerThemeOverlay(appTheme), [appTheme]);
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>({});

  /** 分页模式下：有列定义或已加载行数据即渲染表格 */
  const virtualRowCount = loadedRows.length;
  const hasData = columns.length > 0 || loadedRows.length > 0;
  const gridColumns = useMemo<GridColumn[]>(() => {
    const realColumns = columns.map((col) => ({
      id: col.name,
      title: col.name,
      hasMenu: true,
      width: columnWidths[col.name] ?? Math.max(120, Math.min(280, col.name.length * 8 + 96)),
    }));

    if (realColumns.length >= DATABASE_EDITOR_MIN_COLUMNS) return realColumns;
    return [
      ...realColumns,
      ...Array.from({ length: DATABASE_EDITOR_MIN_COLUMNS - realColumns.length }, (_, index) => ({
        id: `__placeholder_${index}`,
        title: '',
        width: 96,
      })),
    ];
  }, [columnWidths, columns]);

  const handleGridSelectionChange = useCallback((next: GridSelection) => {
    if (isEmptyGridSelection(next)) {
      onSelectionChange(null);
    } else {
      onSelectionChange(next);
    }
  }, [onSelectionChange]);

  const rowMarkers = useMemo(
    () => ({
      kind: 'both' as const,
      checkboxStyle: 'square' as const,
      width: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
      startIndex: pageStartIndex + 1,
      theme: rowMarkerTheme,
    }),
    [pageStartIndex, rowMarkerTheme],
  );

  const getCellContent = useCallback((cell: Item): GridCell => {
    const [col, row] = cell;
    if (col >= columns.length) {
      return {
        kind: GridCellKind.Text,
        allowOverlay: false,
        readonly: true,
        displayData: '',
        data: '',
        style: 'faded',
      };
    }

    const rowData = loadedRows[row];
    if (!rowData) {
      return {
        kind: GridCellKind.Loading,
        allowOverlay: false,
      };
    }

    const value = rowData[col];
    if (typeof value === 'number') {
      return {
        kind: GridCellKind.Number,
        allowOverlay: true,
        displayData: Number.isFinite(value) ? String(value) : '',
        data: Number.isFinite(value) ? value : undefined,
      };
    }

    if (typeof value === 'boolean') {
      return {
        kind: GridCellKind.Boolean,
        allowOverlay: false,
        data: value,
      };
    }

    const displayData = value === null || value === undefined ? 'null' : String(value);
    return {
      kind: GridCellKind.Text,
      allowOverlay: true,
      displayData,
      data: value === null || value === undefined ? '' : String(value),
      style: value === null || value === undefined ? 'faded' : 'normal',
    };
  }, [columns.length, loadedRows]);

  const handleCellEdited = useCallback((cell: Item, newValue: EditableGridCell) => {
    const [col, row] = cell;
    if (col >= columns.length) return;
    void onCommitCellValue(row, col, cellToValue(newValue));
  }, [columns.length, onCommitCellValue]);

  const handleColumnResize = useCallback((column: GridColumn, newSize: number, colIndex: number) => {
    if (colIndex >= columns.length) return;
    const columnId = column.id ?? columns[colIndex]?.name;
    if (!columnId) return;
    setColumnWidths((prev) => ({ ...prev, [columnId]: newSize }));
  }, [columns]);

  if (!hasData) {
    if (loading) {
      return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col items-center justify-center gap-3 bg-card text-muted-foreground">
          <div className="size-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
          <span className="text-xs">{t('databaseEditor.loadingProjectData')}</span>
        </div>
      );
    }
    return (
      <Empty className="min-h-0 min-w-0 rounded-none bg-card">
        <EmptyHeader>
          <EmptyMedia variant="icon" className="size-12 text-muted-foreground">
            <VscDatabase className="size-6" />
          </EmptyMedia>
          <EmptyTitle>{t('databaseEditor.noDataFrameSelected')}</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="relative flex min-h-0 min-w-0 flex-1 overflow-hidden bg-card">
      <DataEditor
        className="h-full w-full"
        width="100%"
        height="100%"
        theme={dataGridTheme}
        columns={gridColumns}
        rows={virtualRowCount}
        getCellContent={getCellContent}
        getCellsForSelection
        rowHeight={DATABASE_EDITOR_ROW_HEIGHT}
        headerHeight={44}
        drawHeader={(args) => {
          const { ctx, rect, column, columnIndex, theme, isHovered, menuBounds } = args;
          if (columnIndex < 0 || columnIndex >= columns.length) return false;
          const col = columns[columnIndex];
          const padX = theme.cellHorizontalPadding ?? 8;
          const availWidth = rect.width - padX * 2;
          if (availWidth <= 0) return true;

          ctx.save();
          ctx.beginPath();
          ctx.rect(rect.x, rect.y, rect.width, rect.height);
          ctx.clip();

          const x = rect.x + padX;
          ctx.textBaseline = 'middle';
          ctx.textAlign = 'left';

          // 列名（主行）
          ctx.fillStyle = theme.textHeader;
          ctx.font = `600 13px ${theme.fontFamily}`;
          ctx.fillText(col.name, x, rect.y + rect.height / 2 - 7, availWidth);

          // 类型（次行，弱化）
          ctx.fillStyle = theme.textLight ?? theme.textMedium ?? theme.textHeader;
          ctx.font = `11px ${theme.fontFamily}`;
          ctx.fillText(col.type, x, rect.y + rect.height / 2 + 8, availWidth);

          // 悬浮时绘制列菜单下拉指示（点击区域由 hasMenu/menuBounds 提供）
          if (column.hasMenu && isHovered && menuBounds) {
            const cx = menuBounds.x + menuBounds.width / 2;
            const cy = menuBounds.y + menuBounds.height / 2;
            ctx.strokeStyle = theme.textLight ?? theme.textMedium ?? theme.textHeader;
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(cx - 4, cy - 2);
            ctx.lineTo(cx, cy + 2);
            ctx.lineTo(cx + 4, cy - 2);
            ctx.stroke();
          }

          ctx.restore();
          return true;
        }}
        rowMarkers={rowMarkers}
        /** 与 Glide 文档一致：auto=鼠标下 Ctrl/Cmd 点行号追加、Shift 扩选连续区间；勿用 multi（等同始终按住 Ctrl） */
        rowSelectionMode="auto"
        rowSelect="multi"
        columnSelect="multi"
        rangeSelect="multi-rect"
        gridSelection={selection ?? emptyGridSelection}
        onGridSelectionChange={handleGridSelectionChange}
        onSelectionCleared={() => onSelectionChange(null)}
        onCellEdited={handleCellEdited}
        onColumnResize={handleColumnResize}
        minColumnWidth={72}
        maxColumnWidth={520}
        cellActivationBehavior="double-click"
        onHeaderMenuClick={(colIndex, bounds) => {
          if (colIndex >= columns.length) return;
          onContextMenu(
            { x: bounds.x, y: bounds.y + bounds.height },
            { type: 'header', colIndex, colName: columns[colIndex]?.name }
          );
        }}
        onHeaderContextMenu={(colIndex, event) => {
          if (colIndex >= columns.length) return;
          event.preventDefault();
          onContextMenu(
            { x: event.bounds.x, y: event.bounds.y + event.bounds.height },
            { type: 'header', colIndex, colName: columns[colIndex]?.name }
          );
        }}
        onCellContextMenu={(cell, event) => {
          const [col, row] = cell;
          if (row >= loadedRows.length) return;
          event.preventDefault();
          if (col < 0) {
            onContextMenu(
              { x: event.bounds.x, y: event.bounds.y + event.bounds.height },
              { type: 'row', rowIndex: row }
            );
            return;
          }
          if (col >= columns.length) return;
          onContextMenu(
            { x: event.bounds.x, y: event.bounds.y + event.bounds.height },
            { type: 'cell', rowIndex: row, colIndex: col, colName: columns[col]?.name }
          );
        }}
        smoothScrollX
        smoothScrollY
        keybindings={{ search: false }}
      />
      {loading && (
        <div className="pointer-events-none absolute bottom-3 right-4 rounded-md border border-border bg-popover/95 px-2.5 py-1.5 text-[11px] font-medium text-popover-foreground shadow-lg backdrop-blur">
          {t('databaseEditor.loadingProjectData')}
        </div>
      )}
    </div>
  );
};

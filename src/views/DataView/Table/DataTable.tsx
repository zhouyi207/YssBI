import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DataEditor,
  GridCellKind,
  GridColumnIcon,
  type EditableGridCell,
  type GridCell,
  type GridColumn,
  type GridSelection,
  type Item,
} from '@glideapps/glide-data-grid';
import '@glideapps/glide-data-grid/dist/index.css';
import { VscDatabase } from 'react-icons/vsc';
import type { ColumnMeta } from '@/features/application/dataView';
import { emptyGridSelection, isEmptyGridSelection } from '@/features/application/dataView';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { buildDataGridThemeOverlay, buildRowMarkerThemeOverlay } from './dataGridTheme';
import {
  DATA_VIEW_ROW_HEIGHT,
  DATA_VIEW_ROW_MARKER_WIDE_WIDTH,
  DATA_VIEW_MIN_COLUMNS,
} from '@/app/appConfig/default';

interface ContextMenuTarget {
  type: 'cell' | 'header' | 'row';
  rowIndex?: number;
  colIndex?: number;
  colName?: string;
}

interface DataTableProps {
  columns: ColumnMeta[];
  loadedRows: any[][];
  pageStartIndex: number;
  loading: boolean;

  // selection：与 Glide `GridSelection` 一致（受控），无独立领域模型
  selection: GridSelection | null;
  onSelectionChange: (selection: GridSelection | null) => void;

  onCommitCellValue: (row: number, col: number, value: unknown) => Promise<void>;

  // context menu
  onContextMenu: (position: { x: number; y: number }, target: ContextMenuTarget) => void;
}

function dtypeToIcon(dtype: string): GridColumnIcon {
  const normalized = dtype.toLowerCase();
  if (normalized.includes('int') || normalized.includes('float') || normalized.includes('double') || normalized.includes('number')) {
    return GridColumnIcon.HeaderNumber;
  }
  if (normalized.includes('bool')) return GridColumnIcon.HeaderBoolean;
  return GridColumnIcon.HeaderString;
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

  /** 分页模式下只渲染当前页数据，避免一次性绘制大数据集 */
  const virtualRowCount = loadedRows.length;
  const hasData = columns.length > 0;
  const gridColumns = useMemo<GridColumn[]>(() => {
    const realColumns = columns.map((col) => ({
      id: col.name,
      title: col.name,
      icon: dtypeToIcon(col.type),
      hasMenu: true,
      width: columnWidths[col.name] ?? Math.max(120, Math.min(280, col.name.length * 8 + 96)),
    }));

    if (realColumns.length >= DATA_VIEW_MIN_COLUMNS) return realColumns;
    return [
      ...realColumns,
      ...Array.from({ length: DATA_VIEW_MIN_COLUMNS - realColumns.length }, (_, index) => ({
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
      width: DATA_VIEW_ROW_MARKER_WIDE_WIDTH,
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
    return (
      <div className="flex min-h-0 min-w-0 flex-1 flex-col items-center justify-center gap-3 bg-card">
        <div className="flex size-14 items-center justify-center rounded-xl border border-border bg-muted text-muted-foreground">
          <VscDatabase size={30} />
        </div>
        <span className="text-sm font-medium text-muted-foreground">
          {loading ? t('dataView.loadingProjectData') : t('dataView.noDataFrameSelected')}
        </span>
      </div>
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
        rowHeight={DATA_VIEW_ROW_HEIGHT}
        headerHeight={36}
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
          {t('dataView.loadingProjectData')}
        </div>
      )}
    </div>
  );
};

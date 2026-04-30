import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CompactSelection,
  DataEditor,
  GridCellKind,
  GridColumnIcon,
  type EditableGridCell,
  type GridCell,
  type GridColumn,
  type GridSelection,
  type Item,
  type Theme,
} from '@glideapps/glide-data-grid';
import '@glideapps/glide-data-grid/dist/index.css';
import { VscDatabase } from 'react-icons/vsc';
import type { ColumnMeta, SelectionRange } from '@/features/application/dataView';
import { selectionBounds } from '@/features/application/dataView';
import { DATA_VIEW_ROW_HEIGHT, DATA_VIEW_ROW_NUM_WIDTH, DATA_VIEW_MIN_COLUMNS } from '@/app/appConfig/default';

interface DataGridThemeTokens {
  accentColor: string;
  accentFg: string;
  accentLight: string;
  workbenchBg: string;
  sidebarBg: string;
  foreground: string;
  mutedForeground: string;
  border: string;
  hover: string;
  active: string;
  rowMarkerBg: string;
  rowMarkerHover: string;
  rowMarkerText: string;
}

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

  // selection
  selection: SelectionRange | null;
  onSelectionChange: (selection: SelectionRange | null) => void;

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

function selectionToGridSelection(selection: SelectionRange | null, columnCount: number, rowCount: number): GridSelection {
  if (!selection) {
    return {
      columns: CompactSelection.empty(),
      rows: CompactSelection.empty(),
    };
  }

  const { r0, r1, c0, c1 } = selectionBounds(selection);
  const fullRowsSelected = columnCount > 0 && c0 === 0 && c1 >= columnCount - 1;
  const fullColumnsSelected = rowCount > 0 && r0 === 0 && r1 >= rowCount - 1;

  return {
    current: {
      cell: [c0, r0],
      range: { x: c0, y: r0, width: c1 - c0 + 1, height: r1 - r0 + 1 },
      rangeStack: [],
    },
    columns: fullColumnsSelected ? CompactSelection.fromSingleSelection([c0, c1 + 1]) : CompactSelection.empty(),
    rows: fullRowsSelected ? CompactSelection.fromSingleSelection([r0, r1 + 1]) : CompactSelection.empty(),
  };
}

function gridSelectionToSelection(gridSelection: GridSelection, columnCount: number, rowCount: number): SelectionRange | null {
  const firstRow = gridSelection.rows.first();
  const lastRow = gridSelection.rows.last();
  if (firstRow !== undefined && lastRow !== undefined && columnCount > 0) {
    return {
      anchor: { row: firstRow, col: 0 },
      end: { row: lastRow, col: columnCount - 1 },
    };
  }

  const firstCol = gridSelection.columns.first();
  const lastCol = gridSelection.columns.last();
  if (firstCol !== undefined && lastCol !== undefined && rowCount > 0) {
    return {
      anchor: { row: 0, col: firstCol },
      end: { row: rowCount - 1, col: lastCol },
    };
  }

  const current = gridSelection.current;
  if (current) {
    const range = current.range;
    return {
      anchor: { row: range.y, col: range.x },
      end: { row: range.y + Math.max(0, range.height - 1), col: range.x + Math.max(0, range.width - 1) },
    };
  }

  return null;
}

function readCssVar(style: CSSStyleDeclaration, name: string, fallback: string): string {
  return style.getPropertyValue(name).trim() || fallback;
}

function toCanvasColor(value: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  if (!context) return fallback;
  context.fillStyle = fallback;
  context.fillStyle = value;
  const resolved = context.fillStyle;
  return resolved && resolved !== fallback ? resolved : fallback;
}

function readDataGridThemeTokens(): DataGridThemeTokens {
  if (typeof window === 'undefined') {
    return {
      accentColor: '#2563eb',
      accentFg: '#ffffff',
      accentLight: 'rgba(37, 99, 235, 0.16)',
      workbenchBg: '#ffffff',
      sidebarBg: '#f8fafc',
      foreground: '#111827',
      mutedForeground: '#6b7280',
      border: '#e5e7eb',
      hover: 'rgba(37, 99, 235, 0.12)',
      active: 'rgba(37, 99, 235, 0.16)',
      rowMarkerBg: '#f8fafc',
      rowMarkerHover: '#e2e8f0',
      rowMarkerText: '#0f172a',
    };
  }

  const style = window.getComputedStyle(document.documentElement);
  const isDark = document.documentElement.classList.contains('dark');
  const lightFallbacks = {
    workbenchBg: '#ffffff',
    sidebarBg: '#f8fafc',
    foreground: '#0f172a',
    mutedForeground: '#64748b',
    border: '#e2e8f0',
    rowMarkerBg: '#f8fafc',
    rowMarkerHover: '#e2e8f0',
  };
  const darkFallbacks = {
    workbenchBg: '#171717',
    sidebarBg: '#262626',
    foreground: '#f8fafc',
    mutedForeground: '#a3a3a3',
    border: 'rgba(255, 255, 255, 0.12)',
    rowMarkerBg: '#262626',
    rowMarkerHover: '#333333',
  };
  const fallbacks = isDark ? darkFallbacks : lightFallbacks;
  const accentColor = toCanvasColor(readCssVar(style, '--accent-color', '#2563eb'), '#2563eb');

  return {
    accentColor,
    accentFg: '#ffffff',
    accentLight: readCssVar(style, '--interactive-active', 'rgba(37, 99, 235, 0.16)'),
    workbenchBg: toCanvasColor(readCssVar(style, '--card', readCssVar(style, '--workbench-bg', fallbacks.workbenchBg)), fallbacks.workbenchBg),
    sidebarBg: toCanvasColor(readCssVar(style, '--muted', readCssVar(style, '--sidebar-bg', fallbacks.sidebarBg)), fallbacks.sidebarBg),
    foreground: toCanvasColor(readCssVar(style, '--workbench-fg', readCssVar(style, '--foreground', fallbacks.foreground)), fallbacks.foreground),
    mutedForeground: toCanvasColor(readCssVar(style, '--text-secondary', readCssVar(style, '--muted-foreground', fallbacks.mutedForeground)), fallbacks.mutedForeground),
    border: toCanvasColor(readCssVar(style, '--strong-border', readCssVar(style, '--border', fallbacks.border)), fallbacks.border),
    hover: readCssVar(style, '--interactive-hover', 'rgba(37, 99, 235, 0.12)'),
    active: readCssVar(style, '--interactive-active', 'rgba(37, 99, 235, 0.16)'),
    rowMarkerBg: fallbacks.rowMarkerBg,
    rowMarkerHover: fallbacks.rowMarkerHover,
    rowMarkerText: fallbacks.foreground,
  };
}

function useDataGridThemeTokens(): DataGridThemeTokens {
  const [tokens, setTokens] = useState(readDataGridThemeTokens);

  useEffect(() => {
    let rafId: number | null = null;
    const update = () => {
      if (rafId !== null) return;
      rafId = window.requestAnimationFrame(() => {
        rafId = null;
        setTokens(readDataGridThemeTokens());
      });
    };

    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class', 'style'],
    });

    return () => {
      if (rafId !== null) window.cancelAnimationFrame(rafId);
      observer.disconnect();
    };
  }, []);

  return tokens;
}

export const DataTable: React.FC<DataTableProps> = ({
  columns, loadedRows, pageStartIndex, loading,
  selection,
  onSelectionChange,
  onCommitCellValue,
  onContextMenu,
}) => {
  const { t } = useTranslation();
  const themeTokens = useDataGridThemeTokens();
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

  const gridSelection = useMemo(
    () => selectionToGridSelection(selection, columns.length, virtualRowCount),
    [columns.length, selection, virtualRowCount]
  );

  const theme = useMemo<Partial<Theme>>(() => ({
    accentColor: themeTokens.accentColor,
    accentFg: themeTokens.accentFg,
    accentLight: themeTokens.accentLight,
    bgCell: themeTokens.workbenchBg,
    bgCellMedium: themeTokens.sidebarBg,
    bgHeader: themeTokens.sidebarBg,
    bgHeaderHovered: themeTokens.hover,
    bgHeaderHasFocus: themeTokens.accentColor,
    bgBubble: themeTokens.sidebarBg,
    bgBubbleSelected: themeTokens.accentColor,
    textDark: themeTokens.foreground,
    textMedium: themeTokens.mutedForeground,
    textLight: themeTokens.mutedForeground,
    textBubble: themeTokens.foreground,
    textHeader: themeTokens.foreground,
    textHeaderSelected: themeTokens.accentFg,
    bgIconHeader: themeTokens.sidebarBg,
    fgIconHeader: themeTokens.mutedForeground,
    borderColor: themeTokens.border,
    horizontalBorderColor: themeTokens.border,
    headerBottomBorderColor: themeTokens.border,
    linkColor: themeTokens.accentColor,
    fontFamily: 'var(--font-sans)',
    baseFontStyle: '11px var(--font-sans)',
    headerFontStyle: '600 11px var(--font-sans)',
    editorFontSize: '11px',
  }), [themeTokens]);

  const rowMarkerTheme = useMemo<Partial<Theme>>(() => ({
    bgCell: themeTokens.rowMarkerBg,
    bgCellMedium: themeTokens.rowMarkerBg,
    bgHeader: themeTokens.rowMarkerBg,
    bgHeaderHovered: themeTokens.rowMarkerHover,
    bgHeaderHasFocus: themeTokens.active,
    accentLight: themeTokens.active,
    textLight: themeTokens.rowMarkerText,
    textMedium: themeTokens.rowMarkerText,
  }), [themeTokens]);

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

  const handleGridSelectionChange = useCallback((next: GridSelection) => {
    onSelectionChange(gridSelectionToSelection(next, columns.length, virtualRowCount));
  }, [columns.length, onSelectionChange, virtualRowCount]);

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
        columns={gridColumns}
        rows={virtualRowCount}
        getCellContent={getCellContent}
        getCellsForSelection
        rowHeight={DATA_VIEW_ROW_HEIGHT}
        headerHeight={36}
        rowMarkers={{
          kind: 'clickable-number',
          width: DATA_VIEW_ROW_NUM_WIDTH,
          startIndex: pageStartIndex + 1,
          theme: rowMarkerTheme,
        }}
        rowSelect="multi"
        columnSelect="multi"
        rangeSelect="multi-rect"
        gridSelection={gridSelection}
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
        theme={theme}
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

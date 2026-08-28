import React, { useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CellStyleModule,
  ClientSideRowModelModule,
  RenderApiModule,
  RowApiModule,
  RowSelectionModule,
  type CellStyle,
  type ColDef,
  type GetRowIdParams,
} from 'ag-grid-community';
import { AgGridReact } from 'ag-grid-react';
import { VscDatabase } from 'react-icons/vsc';
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { buildAgGridTheme } from '@/components/data-grid/agGridTheme';
import type { DatabaseGridSelection } from '@/features/application/databaseEditor';
import { useSettingsStore } from '@/features/application/viewCapabilities';
import { resolveThemeTokens } from '@/shared/theme/themeTokens';
import type { ColumnInfo, DatabaseRow } from '@/shared/types/dto/database';
import {
  DATABASE_EDITOR_MIN_COLUMNS,
  DATABASE_EDITOR_ROW_HEIGHT,
  DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
} from '@/app/appConfig/default';
import {
  DatabaseCellRenderer,
  DatabaseColumnHeader,
  DatabaseRowMarker,
} from './DatabaseGridRenderers';
import {
  dataColumnId,
  isGridCellActive,
  isGridCellSelected,
  isGridColumnSelected,
  type DatabaseGridRow,
} from './databaseGridModel';
import { databaseGridSelectionToClipboardText } from './databaseGridClipboard';
import { useDatabaseGridSelectionAdapter } from './useDatabaseGridSelectionAdapter';

const ROW_MARKER_COLUMN_ID = '__row_marker__';

const GRID_MODULES = [CellStyleModule, ClientSideRowModelModule, RenderApiModule, RowApiModule, RowSelectionModule];

const ROW_SELECTION = {
  mode: 'multiRow',
  checkboxes: false,
  headerCheckbox: false,
  enableClickSelection: false,
  ctrlASelectsRows: false,
} as const;

const BASE_COLUMN_DEF: ColDef<DatabaseGridRow> = {
  cellDataType: false,
  filter: false,
  resizable: true,
  sortable: false,
  suppressHeaderMenuButton: true,
  suppressMovable: true,
};

interface DataTableProps {
  columns: ColumnInfo[];
  loadedRows: DatabaseRow[];
  loadedRowIds: number[];
  pageStartIndex: number;
  loading: boolean;
  selection: DatabaseGridSelection | null;
  onSelectionChange: (selection: DatabaseGridSelection | null) => void;
}

function getRowId({ data }: GetRowIdParams<DatabaseGridRow>): string {
  return String(data.rowId);
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement
    && (target.isContentEditable
      || target instanceof HTMLInputElement
      || target instanceof HTMLTextAreaElement
      || target instanceof HTMLSelectElement);
}

export const DataTable: React.FC<DataTableProps> = ({
  columns,
  loadedRows,
  loadedRowIds,
  pageStartIndex,
  loading,
  selection,
  onSelectionChange,
}) => {
  const { t } = useTranslation();
  const appTheme = useSettingsStore((state) => state.theme);
  const themeTokens = useMemo(() => resolveThemeTokens(appTheme), [appTheme]);
  const gridRef = useRef<AgGridReact<DatabaseGridRow>>(null);
  const dataGridTheme = useMemo(() => buildAgGridTheme(appTheme), [appTheme]);
  const gridRows = useMemo<DatabaseGridRow[]>(() => loadedRows.map((values, rowIndex) => ({
    values,
    rowId: loadedRowIds[rowIndex] ?? `page:${pageStartIndex + rowIndex}`,
    sourceRowIndex: rowIndex,
  })), [loadedRows, loadedRowIds, pageStartIndex]);

  const {
    selectionRef,
    captureCellKeyboard,
    handleCellFocused,
    handleCellMouseDown,
    handleCellMouseOver,
    handleColumnSelect,
    handleGridReady,
    handleRowSelect,
    handleSelectionChanged,
  } = useDatabaseGridSelectionAdapter({
    gridRef,
    selection,
    rowCount: loadedRows.length,
    columnCount: columns.length,
    rowData: gridRows,
    onSelectionChange,
  });

  const defaultColumnDef = useMemo<ColDef<DatabaseGridRow>>(() => ({
    ...BASE_COLUMN_DEF,
    suppressKeyboardEvent: ({ editing, event }) => {
      if (editing) return false;
      captureCellKeyboard(event);
      const commandKey = event.ctrlKey || event.metaKey;
      return commandKey && event.key.toLowerCase() === 'a';
    },
  }), [captureCellKeyboard]);

  const cellSelectionStyle = useCallback((
    rowIndex: number,
    columnIndex: number,
  ): CellStyle => {
    const current = selectionRef.current;
    const selected = isGridCellSelected(current, rowIndex, columnIndex);
    const active = isGridCellActive(current, rowIndex, columnIndex);
    return {
      backgroundColor: selected
        ? `color-mix(in srgb, ${themeTokens.selection} 16%, transparent)`
        : 'transparent',
      boxShadow: active ? `inset 0 0 0 1px ${themeTokens.accent}` : 'none',
    };
  }, [themeTokens.accent, themeTokens.selection]);

  const gridColumns = useMemo<ColDef<DatabaseGridRow>[]>(() => {
    const realColumns = columns.map<ColDef<DatabaseGridRow>>((column, columnIndex) => ({
      colId: dataColumnId(columnIndex),
      editable: false,
      headerComponent: DatabaseColumnHeader,
      headerComponentParams: {
        columnIndex,
        columnType: column.type,
        isSelected: () => isGridColumnSelected(selectionRef.current, columnIndex),
        onSelect: handleColumnSelect,
      },
      headerName: column.name,
      initialWidth: Math.max(120, Math.min(280, column.name.length * 8 + 96)),
      minWidth: 72,
      maxWidth: 520,
      valueGetter: ({ data }) => data?.values[columnIndex],
      cellRenderer: DatabaseCellRenderer,
      cellStyle: ({ data }) => data
        ? cellSelectionStyle(data.sourceRowIndex, columnIndex)
        : { backgroundColor: 'transparent', boxShadow: 'none' },
    }));

    const placeholderColumns = Array.from(
      { length: Math.max(0, DATABASE_EDITOR_MIN_COLUMNS - realColumns.length) },
      (_, index): ColDef<DatabaseGridRow> => ({
        colId: `__placeholder_${index}`,
        editable: false,
        headerName: '',
        initialWidth: 96,
        suppressNavigable: true,
      }),
    );

    return [
      {
        colId: ROW_MARKER_COLUMN_ID,
        cellRenderer: DatabaseRowMarker,
        cellRendererParams: { onSelectRow: handleRowSelect },
        cellStyle: { padding: 0 },
        editable: false,
        headerName: '',
        lockPinned: true,
        lockPosition: 'left',
        maxWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        minWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        pinned: 'left',
        resizable: false,
        suppressNavigable: true,
        valueGetter: ({ data }) => data
          ? pageStartIndex + data.sourceRowIndex + 1
          : undefined,
        width: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
      },
      ...realColumns,
      ...placeholderColumns,
    ];
  }, [
    columns,
    pageStartIndex,
    cellSelectionStyle,
    handleColumnSelect,
    handleRowSelect,
  ]);

  const handleCopy = useCallback((event: React.ClipboardEvent<HTMLDivElement>) => {
    if (isTextEntryTarget(event.target)) return;
    const text = databaseGridSelectionToClipboardText(
      selectionRef.current,
      loadedRows,
      columns.length,
    );
    if (text === null) return;
    event.preventDefault();
    event.clipboardData.setData('text/plain', text);
  }, [columns.length, loadedRows]);

  const hasData = columns.length > 0 || loadedRows.length > 0;
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
    <div
      className="relative flex min-h-0 min-w-0 flex-1 overflow-hidden bg-card"
      onCopy={handleCopy}
    >
      <AgGridReact<DatabaseGridRow>
        ref={gridRef}
        animateRows={false}
        className="h-full w-full"
        columnDefs={gridColumns}
        defaultColDef={defaultColumnDef}
        getRowId={getRowId}
        headerHeight={44}
        modules={GRID_MODULES}
        onCellFocused={handleCellFocused}
        onCellMouseDown={handleCellMouseDown}
        onCellMouseOver={handleCellMouseOver}
        onGridReady={handleGridReady}
        onSelectionChanged={handleSelectionChanged}
        rowData={gridRows}
        rowHeight={DATABASE_EDITOR_ROW_HEIGHT}
        rowSelection={ROW_SELECTION}
        stopEditingWhenCellsLoseFocus
        suppressNoRowsOverlay
        theme={dataGridTheme}
      />
      {loading ? (
        <div className="pointer-events-none absolute bottom-3 right-4 rounded-md border border-border bg-popover/95 px-2.5 py-1.5 text-[11px] font-medium text-popover-foreground shadow-lg backdrop-blur">
          {t('databaseEditor.loadingProjectData')}
        </div>
      ) : null}
    </div>
  );
};

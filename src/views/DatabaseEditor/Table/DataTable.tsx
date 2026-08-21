import React, { useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CellStyleModule,
  CheckboxEditorModule,
  ClientSideRowModelModule,
  NumberEditorModule,
  RenderApiModule,
  RowApiModule,
  RowSelectionModule,
  TextEditorModule,
  type CellContextMenuEvent,
  type CellEditRequestEvent,
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
import type {
  DatabaseCellBatchMutationOutcome,
  DatabaseCellEditInput,
  DatabaseGridSelection,
} from '@/features/application/databaseEditor';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
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
  cellEditorForColumnType,
} from './DatabaseGridRenderers';
import {
  dataColumnId,
  dataColumnIndexFromId,
  isGridCellActive,
  isGridCellSelected,
  isGridColumnSelected,
  type DatabaseGridRow,
} from './databaseGridModel';
import {
  databaseGridSelectionToClipboardText,
  parseDatabaseGridClipboard,
} from './databaseGridClipboard';
import { useDatabaseGridSelectionAdapter } from './useDatabaseGridSelectionAdapter';

const ROW_MARKER_COLUMN_ID = '__row_marker__';

const GRID_MODULES = [
  CellStyleModule,
  CheckboxEditorModule,
  ClientSideRowModelModule,
  NumberEditorModule,
  RenderApiModule,
  RowApiModule,
  RowSelectionModule,
  TextEditorModule,
];

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

interface ContextMenuTarget {
  type: 'cell' | 'header' | 'row';
  rowIndex?: number;
  colIndex?: number;
  colName?: string;
}

interface DataTableProps {
  columns: ColumnInfo[];
  loadedRows: DatabaseRow[];
  loadedRowIds: number[];
  pageStartIndex: number;
  loading: boolean;
  selection: DatabaseGridSelection | null;
  onSelectionChange: (selection: DatabaseGridSelection | null) => void;
  onCommitCellValue: (row: number, col: number, value: unknown) => Promise<void>;
  onCommitCellValues: (
    edits: readonly DatabaseCellEditInput[],
  ) => Promise<DatabaseCellBatchMutationOutcome>;
  onContextMenu: (position: { x: number; y: number }, target: ContextMenuTarget) => void;
}

function getRowId({ data }: GetRowIdParams<DatabaseGridRow>): string {
  return String(data.rowId);
}

function browserMouseEvent(event: Event | null | undefined): MouseEvent | null {
  return event instanceof MouseEvent ? event : null;
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
  onCommitCellValue,
  onCommitCellValues,
  onContextMenu,
}) => {
  const { t } = useTranslation();
  const appTheme = useSettingsStore((state) => state.theme);
  const gridRef = useRef<AgGridReact<DatabaseGridRow>>(null);
  const pasteInFlightRef = useRef(false);

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
      return (commandKey && event.key.toLowerCase() === 'a') || event.key === 'Delete';
    },
  }), [captureCellKeyboard]);

  const handleHeaderMenu = useCallback((
    position: { x: number; y: number },
    columnIndex: number,
  ) => {
    const column = columns[columnIndex];
    if (!column) return;
    onContextMenu(position, {
      type: 'header',
      colIndex: columnIndex,
      colName: column.name,
    });
  }, [columns, onContextMenu]);

  const cellSelectionStyle = useCallback((
    rowIndex: number,
    columnIndex: number,
  ): CellStyle => {
    const current = selectionRef.current;
    const selected = isGridCellSelected(current, rowIndex, columnIndex);
    const active = isGridCellActive(current, rowIndex, columnIndex);
    return {
      backgroundColor: selected
        ? `color-mix(in srgb, ${appTheme.selectionRegion} 16%, transparent)`
        : 'transparent',
      boxShadow: active ? `inset 0 0 0 1px ${appTheme.accentColor}` : 'none',
    };
  }, [appTheme.accentColor, appTheme.selectionRegion]);

  const gridColumns = useMemo<ColDef<DatabaseGridRow>[]>(() => {
    const realColumns = columns.map<ColDef<DatabaseGridRow>>((column, columnIndex) => ({
      colId: dataColumnId(columnIndex),
      editable: true,
      headerComponent: DatabaseColumnHeader,
      headerComponentParams: {
        columnIndex,
        columnType: column.type,
        isSelected: () => isGridColumnSelected(selectionRef.current, columnIndex),
        onOpenMenu: handleHeaderMenu,
        onSelect: handleColumnSelect,
      },
      headerName: column.name,
      initialWidth: Math.max(120, Math.min(280, column.name.length * 8 + 96)),
      minWidth: 72,
      maxWidth: 520,
      valueGetter: ({ data }) => data?.values[columnIndex],
      cellEditor: cellEditorForColumnType(column.type),
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
    handleHeaderMenu,
    handleRowSelect,
  ]);


  const handleCellEditRequest = useCallback((event: CellEditRequestEvent<DatabaseGridRow>) => {
    const columnIndex = dataColumnIndexFromId(event.column.getColId());
    if (columnIndex === null) return;
    void onCommitCellValue(event.data.sourceRowIndex, columnIndex, event.newValue);
  }, [onCommitCellValue]);

  const handleCellContextMenu = useCallback((event: CellContextMenuEvent<DatabaseGridRow>) => {
    event.event?.preventDefault();
    const mouseEvent = browserMouseEvent(event.event);
    if (!mouseEvent || !event.data) return;
    const rowIndex = event.data.sourceRowIndex;
    const columnId = event.column.getColId();
    const position = { x: mouseEvent.clientX, y: mouseEvent.clientY };
    if (columnId === ROW_MARKER_COLUMN_ID) {
      onContextMenu(position, { type: 'row', rowIndex });
      return;
    }
    const columnIndex = dataColumnIndexFromId(columnId);
    const column = columnIndex === null ? undefined : columns[columnIndex];
    if (!column || columnIndex === null) return;
    onContextMenu(position, {
      type: 'cell',
      rowIndex,
      colIndex: columnIndex,
      colName: column.name,
    });
  }, [columns, onContextMenu]);

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

  const handlePaste = useCallback((event: React.ClipboardEvent<HTMLDivElement>) => {
    if (isTextEntryTarget(event.target) || pasteInFlightRef.current) return;
    const current = selectionRef.current;
    if (current?.type !== 'cells') return;
    const values = parseDatabaseGridClipboard(event.clipboardData.getData('text/plain'));
    if (values.length === 0) return;
    event.preventDefault();

    const start = current.activeCell;
    const edits: DatabaseCellEditInput[] = [];
    for (let rowOffset = 0; rowOffset < values.length; rowOffset += 1) {
      const rowIndex = start.row + rowOffset;
      if (rowIndex >= loadedRows.length) break;
      const row = values[rowOffset] ?? [];
      for (let columnOffset = 0; columnOffset < row.length; columnOffset += 1) {
        const columnIndex = start.column + columnOffset;
        if (columnIndex >= columns.length) break;
        edits.push({ row: rowIndex, column: columnIndex, value: row[columnOffset] });
      }
    }
    if (edits.length === 0) return;

    pasteInFlightRef.current = true;
    void onCommitCellValues(edits).finally(() => {
      pasteInFlightRef.current = false;
    });
  }, [columns.length, loadedRows.length, onCommitCellValues]);

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
      onPaste={handlePaste}
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
        onCellContextMenu={handleCellContextMenu}
        onCellEditRequest={handleCellEditRequest}
        onCellFocused={handleCellFocused}
        onCellMouseDown={handleCellMouseDown}
        onCellMouseOver={handleCellMouseOver}
        onGridReady={handleGridReady}
        onSelectionChanged={handleSelectionChanged}
        preventDefaultOnContextMenu
        readOnlyEdit
        rowData={gridRows}
        rowHeight={DATABASE_EDITOR_ROW_HEIGHT}
        rowSelection={ROW_SELECTION}
        stopEditingWhenCellsLoseFocus
        suppressContextMenu
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

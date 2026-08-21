import { useMemo } from 'react';
import { ClientSideRowModelModule, type ColDef } from 'ag-grid-community';
import {
  AgGridReact,
  type CustomCellRendererProps,
  type CustomHeaderProps,
} from 'ag-grid-react';
import { buildAgGridTheme } from '@/components/data-grid/agGridTheme';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import {
  DATABASE_EDITOR_MIN_COLUMNS,
  DATABASE_EDITOR_ROW_HEIGHT,
  DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
} from '@/app/appConfig/default';

export interface ReadOnlyColumnMeta {
  name: string;
  type?: string;
}

interface ReadOnlyDataGridProps {
  columns: ReadOnlyColumnMeta[];
  rows: unknown[][];
  pageStartIndex?: number;
  loading?: boolean;
  height?: number | string;
  fillHeight?: boolean;
}

type GridRow = unknown[];
type ColumnDataKind = 'number' | 'boolean' | 'string';

type ReadOnlyHeaderProps = CustomHeaderProps<GridRow> & {
  columnType?: string;
};

const GRID_MODULES = [ClientSideRowModelModule];

const DEFAULT_COLUMN_DEF: ColDef<GridRow> = {
  cellDataType: false,
  editable: false,
  filter: false,
  resizable: true,
  sortable: false,
  suppressHeaderMenuButton: true,
  suppressMovable: true,
};

function dtypeToKind(dtype?: string): ColumnDataKind {
  const normalized = (dtype ?? '').toLowerCase();
  if (
    normalized.includes('int') ||
    normalized.includes('float') ||
    normalized.includes('double') ||
    normalized.includes('number')
  ) {
    return 'number';
  }
  if (normalized.includes('bool')) return 'boolean';
  return 'string';
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function ReadOnlyColumnHeader({ displayName, columnType }: ReadOnlyHeaderProps) {
  const kind = dtypeToKind(columnType);
  const typeLabel = columnType || kind;
  const typeMarker = kind === 'number' ? '123' : kind === 'boolean' ? '✓' : 'ABC';

  return (
    <div
      className="flex h-full min-w-0 items-center gap-1.5"
      title={`${displayName} (${typeLabel})`}
      aria-label={`${displayName}, ${typeLabel}`}
    >
      <span
        aria-hidden="true"
        className="flex h-4 min-w-5 shrink-0 items-center justify-center rounded-sm border border-border px-1 text-[8px] font-semibold leading-none text-muted-foreground"
      >
        {typeMarker}
      </span>
      <span className="truncate">{displayName}</span>
    </div>
  );
}

function ReadOnlyCellRenderer({ value }: CustomCellRendererProps<GridRow, unknown>) {
  if (typeof value === 'boolean') {
    return (
      <span className="inline-flex h-full w-full items-center justify-center">
        <span
          aria-hidden="true"
          className={[
            'inline-flex size-3.5 items-center justify-center rounded-[3px] border text-[10px] leading-none',
            value
              ? 'border-primary bg-primary text-primary-foreground'
              : 'border-muted-foreground/60 bg-transparent',
          ].join(' ')}
        >
          {value ? '✓' : null}
        </span>
        <span className="sr-only">{String(value)}</span>
      </span>
    );
  }

  return (
    <span
      className={[
        'block w-full truncate',
        typeof value === 'number' ? 'text-right tabular-nums' : '',
        value === null || value === undefined ? 'text-muted-foreground' : '',
      ].join(' ')}
    >
      {formatCell(value)}
    </span>
  );
}

function RowNumberCellRenderer({ value }: CustomCellRendererProps<GridRow, number>) {
  return (
    <span className="block w-full text-right tabular-nums text-muted-foreground">
      {value}
    </span>
  );
}

function LoadingOverlay() {
  return (
    <div className="pointer-events-none rounded-md border border-border bg-popover/95 px-2.5 py-1.5 text-[11px] font-medium text-popover-foreground shadow-lg backdrop-blur">
      Loading…
    </div>
  );
}

export function ReadOnlyDataGrid({
  columns,
  rows,
  pageStartIndex = 0,
  loading = false,
  height = 480,
  fillHeight = false,
}: ReadOnlyDataGridProps) {
  const appTheme = useSettingsStore((s) => s.theme);

  const dataGridTheme = useMemo(() => buildAgGridTheme(appTheme), [appTheme]);

  const gridColumns = useMemo<ColDef<GridRow>[]>(() => {
    const realColumns = columns.map<ColDef<GridRow>>((column, columnIndex) => ({
      colId: `data_${columnIndex}`,
      headerComponent: ReadOnlyColumnHeader,
      headerComponentParams: { columnType: column.type },
      headerName: column.name,
      width: Math.max(120, Math.min(280, column.name.length * 8 + 96)),
      valueGetter: ({ data }) => data?.[columnIndex],
      cellRenderer: ReadOnlyCellRenderer,
    }));

    const placeholderCount = Math.max(0, DATABASE_EDITOR_MIN_COLUMNS - realColumns.length);
    const placeholderColumns: ColDef<GridRow>[] = Array.from(
      { length: placeholderCount },
      (_, index) => ({
        colId: `__placeholder_${index}`,
        headerName: '',
        width: 96,
      }),
    );

    return [
      {
        colId: '__row_number__',
        headerName: '',
        lockPinned: true,
        lockPosition: 'left',
        maxWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        minWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        pinned: 'left',
        resizable: false,
        suppressNavigable: true,
        width: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        valueGetter: ({ node }) => pageStartIndex + (node?.rowIndex ?? 0) + 1,
        cellRenderer: RowNumberCellRenderer,
      },
      ...realColumns,
      ...placeholderColumns,
    ];
  }, [columns, pageStartIndex]);

  return (
    <div
      className={[
        'relative overflow-hidden rounded-lg border border-border bg-card',
        fillHeight ? 'h-full min-h-60' : '',
      ].join(' ')}
      style={fillHeight ? undefined : { height }}
    >
      <AgGridReact<GridRow>
        animateRows={false}
        className="h-full w-full"
        columnDefs={gridColumns}
        defaultColDef={DEFAULT_COLUMN_DEF}
        headerHeight={36}
        loading={loading}
        loadingOverlayComponent={LoadingOverlay}
        modules={GRID_MODULES}
        rowData={rows}
        rowHeight={DATABASE_EDITOR_ROW_HEIGHT}
        suppressNoRowsOverlay
        theme={dataGridTheme}
      />
    </div>
  );
}

import { useCallback, useMemo } from 'react';
import {
  DataEditor,
  GridCellKind,
  GridColumnIcon,
  type GridCell,
  type GridColumn,
  type Item,
} from '@glideapps/glide-data-grid';
import '@glideapps/glide-data-grid/dist/index.css';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import {
  DATA_VIEW_MIN_COLUMNS,
  DATA_VIEW_ROW_HEIGHT,
  DATA_VIEW_ROW_MARKER_WIDE_WIDTH,
} from '@/app/appConfig/default';
import {
  buildDataGridThemeOverlay,
  buildRowMarkerThemeOverlay,
} from '@/views/DataView/Table/dataGridTheme';

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
}

function dtypeToIcon(dtype?: string): GridColumnIcon {
  const normalized = (dtype ?? '').toLowerCase();
  if (
    normalized.includes('int') ||
    normalized.includes('float') ||
    normalized.includes('double') ||
    normalized.includes('number')
  ) {
    return GridColumnIcon.HeaderNumber;
  }
  if (normalized.includes('bool')) return GridColumnIcon.HeaderBoolean;
  return GridColumnIcon.HeaderString;
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

export function ReadOnlyDataGrid({
  columns,
  rows,
  pageStartIndex = 0,
  loading = false,
  height = 480,
}: ReadOnlyDataGridProps) {
  const appTheme = useSettingsStore((s) => s.theme);
  const dataGridTheme = useMemo(() => buildDataGridThemeOverlay(appTheme), [appTheme]);
  const rowMarkerTheme = useMemo(() => buildRowMarkerThemeOverlay(appTheme), [appTheme]);

  const gridColumns = useMemo<GridColumn[]>(() => {
    const realColumns = columns.map((col) => ({
      id: col.name,
      title: col.name,
      icon: dtypeToIcon(col.type),
      width: Math.max(120, Math.min(280, col.name.length * 8 + 96)),
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
  }, [columns]);

  const getCellContent = useCallback(
    (cell: Item): GridCell => {
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

      const rowData = rows[row];
      if (!rowData) {
        return { kind: GridCellKind.Loading, allowOverlay: false };
      }

      const value = rowData[col];
      const displayData = formatCell(value);
      if (typeof value === 'number') {
        return {
          kind: GridCellKind.Number,
          allowOverlay: false,
          readonly: true,
          displayData: Number.isFinite(value) ? String(value) : displayData,
          data: Number.isFinite(value) ? value : undefined,
        };
      }
      if (typeof value === 'boolean') {
        return { kind: GridCellKind.Boolean, allowOverlay: false, readonly: true, data: value };
      }
      return {
        kind: GridCellKind.Text,
        allowOverlay: false,
        readonly: true,
        displayData,
        data: displayData,
        style: value === null || value === undefined ? 'faded' : 'normal',
      };
    },
    [columns.length, rows],
  );

  const rowMarkers = useMemo(
    () => ({
      kind: 'number' as const,
      width: DATA_VIEW_ROW_MARKER_WIDE_WIDTH,
      startIndex: pageStartIndex + 1,
      theme: rowMarkerTheme,
    }),
    [pageStartIndex, rowMarkerTheme],
  );

  return (
    <div
      className="relative overflow-hidden rounded-lg border border-border bg-card"
      style={{ height }}
    >
      <DataEditor
        className="h-full w-full"
        width="100%"
        height="100%"
        theme={dataGridTheme}
        columns={gridColumns}
        rows={rows.length}
        getCellContent={getCellContent}
        rowHeight={DATA_VIEW_ROW_HEIGHT}
        headerHeight={36}
        rowMarkers={rowMarkers}
        smoothScrollX
        smoothScrollY
        keybindings={{ search: false }}
      />
      {loading ? (
        <div className="pointer-events-none absolute bottom-3 right-4 rounded-md border border-border bg-popover/95 px-2.5 py-1.5 text-[11px] font-medium text-popover-foreground shadow-lg backdrop-blur">
          Loading…
        </div>
      ) : null}
    </div>
  );
}

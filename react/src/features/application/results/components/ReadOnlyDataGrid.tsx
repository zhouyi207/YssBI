import { useMemo } from "react";
import { CellStyleModule, ClientSideRowModelModule, type ColDef } from "ag-grid-community";
import { AgGridReact, type CustomCellRendererProps, type CustomHeaderProps } from "ag-grid-react";
import { buildAgGridTheme } from "@/components/data-grid/agGridTheme";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { resolveColorThemePreset } from "@/shared/theme/colorThemePresets";
import {
  DATABASE_EDITOR_MIN_COLUMNS,
  DATABASE_EDITOR_ROW_HEIGHT,
  DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
} from "@/shared/config-default";

export interface ReadOnlyColumnMeta {
  readonly name: string;
  readonly type?: string;
}

interface ReadOnlyDataGridProps {
  columns: readonly ReadOnlyColumnMeta[];
  rows: readonly (readonly unknown[])[];
  pageStartIndex?: number;
  loading?: boolean;
  height?: number | string;
  fillHeight?: boolean;
  variant?: "default" | "compact";
}

type GridRow = readonly unknown[];
type ColumnDataKind = "number" | "boolean" | "string";

type ReadOnlyHeaderProps = CustomHeaderProps<GridRow> & {
  columnType?: string;
  compact: boolean;
};

const GRID_MODULES = [ClientSideRowModelModule, CellStyleModule];
const COMPACT_ROW_HEIGHT = 28;
const COMPACT_HEADER_HEIGHT = 28;

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
  const normalized = (dtype ?? "").toLowerCase();
  if (
    normalized === "numeric" ||
    normalized.includes("int") ||
    normalized.includes("float") ||
    normalized.includes("double") ||
    normalized.includes("decimal") ||
    normalized.includes("number")
  ) {
    return "number";
  }
  if (normalized.includes("bool")) return "boolean";
  return "string";
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function ReadOnlyColumnHeader({ displayName, columnType, compact }: ReadOnlyHeaderProps) {
  const kind = dtypeToKind(columnType);
  const typeLabel = columnType || kind;
  const typeMarker = kind === "number" ? "123" : kind === "boolean" ? "✓" : "ABC";

  return (
    <div
      className="flex h-full min-w-0 items-center gap-1.5"
      title={compact ? displayName : `${displayName} (${typeLabel})`}
      aria-label={compact ? displayName : `${displayName}, ${typeLabel}`}
    >
      {!compact && (
        <span
          aria-hidden="true"
          className="flex h-4 min-w-5 shrink-0 items-center justify-center rounded-sm border border-border px-1 text-[8px] font-semibold leading-none text-muted-foreground"
        >
          {typeMarker}
        </span>
      )}
      <span className="truncate">{displayName}</span>
    </div>
  );
}

function ReadOnlyCellRenderer({ value }: CustomCellRendererProps<GridRow, unknown>) {
  if (typeof value === "boolean") {
    return (
      <span className="inline-flex h-full w-full items-center justify-center">
        <span
          aria-hidden="true"
          className={[
            "inline-flex size-3.5 items-center justify-center rounded-[3px] border text-[10px] leading-none",
            value
              ? "border-primary bg-primary text-primary-foreground"
              : "border-muted-foreground/60 bg-transparent",
          ].join(" ")}
        >
          {value ? "✓" : null}
        </span>
        <span className="sr-only">{String(value)}</span>
      </span>
    );
  }

  const text = formatCell(value);
  return (
    <span
      title={text}
      className={[
        "block w-full truncate",
        typeof value === "number" ? "text-right tabular-nums" : "",
        value === null || value === undefined ? "text-muted-foreground" : "",
      ].join(" ")}
    >
      {text}
    </span>
  );
}

function RowNumberCellRenderer({ value }: CustomCellRendererProps<GridRow, number>) {
  return (
    <span className="block w-full text-right tabular-nums text-muted-foreground">{value}</span>
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
  height,
  fillHeight = false,
  variant = "default",
}: ReadOnlyDataGridProps) {
  const appTheme = useSettingsStore((s) => resolveColorThemePreset(s.appearance.colorTheme));
  const compact = variant === "compact";
  // Keep a bounded viewport for row virtualisation and reserve room for horizontal scrolling.
  const gridHeight =
    height ??
    (compact
      ? COMPACT_HEADER_HEIGHT + Math.min(8, Math.max(1, rows.length)) * COMPACT_ROW_HEIGHT + 20
      : 480);

  const dataGridTheme = useMemo(() => buildAgGridTheme(appTheme), [appTheme]);
  const gridRows = useMemo(() => [...rows], [rows]);

  const gridColumns = useMemo<ColDef<GridRow>[]>(() => {
    const realColumns = columns.map<ColDef<GridRow>>((column, columnIndex) => ({
      colId: `data_${columnIndex}`,
      headerComponent: ReadOnlyColumnHeader,
      headerComponentParams: { columnType: column.type, compact },
      headerName: column.name,
      minWidth: compact ? 96 : undefined,
      width: Math.max(120, Math.min(280, column.name.length * 8 + (compact ? 32 : 96))),
      valueGetter: ({ data }) => data?.[columnIndex],
      cellRenderer: ReadOnlyCellRenderer,
      cellClass: dtypeToKind(column.type) === "number" ? "text-right tabular-nums" : undefined,
    }));

    if (compact) return realColumns;

    const placeholderCount = Math.max(0, DATABASE_EDITOR_MIN_COLUMNS - realColumns.length);
    const placeholderColumns: ColDef<GridRow>[] = Array.from(
      { length: placeholderCount },
      (_, index) => ({
        colId: `__placeholder_${index}`,
        headerName: "",
        width: 96,
      }),
    );

    return [
      {
        colId: "__row_number__",
        headerName: "",
        lockPinned: true,
        lockPosition: "left",
        maxWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        minWidth: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        pinned: "left",
        resizable: false,
        suppressNavigable: true,
        width: DATABASE_EDITOR_ROW_MARKER_WIDE_WIDTH,
        valueGetter: ({ node }) => pageStartIndex + (node?.rowIndex ?? 0) + 1,
        cellRenderer: RowNumberCellRenderer,
      },
      ...realColumns,
      ...placeholderColumns,
    ];
  }, [columns, compact, pageStartIndex]);

  return (
    <div
      className={[
        "relative min-w-0 w-full max-w-full overflow-hidden border border-border bg-card",
        compact ? "rounded-none" : "rounded-lg",
        fillHeight ? "h-full min-h-60" : "",
      ].join(" ")}
      style={fillHeight ? undefined : { height: gridHeight }}
    >
      <AgGridReact<GridRow>
        animateRows={false}
        className="h-full w-full"
        columnDefs={gridColumns}
        defaultColDef={DEFAULT_COLUMN_DEF}
        headerHeight={compact ? COMPACT_HEADER_HEIGHT : 36}
        loading={loading}
        loadingOverlayComponent={LoadingOverlay}
        modules={GRID_MODULES}
        rowData={gridRows}
        rowHeight={compact ? COMPACT_ROW_HEIGHT : DATABASE_EDITOR_ROW_HEIGHT}
        suppressNoRowsOverlay
        theme={dataGridTheme}
      />
    </div>
  );
}

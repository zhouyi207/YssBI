import type { MouseEvent as ReactMouseEvent } from "react";
import { type CustomCellRendererProps, type CustomHeaderProps } from "ag-grid-react";
import { type DatabaseGridRow, type DatabaseGridSelectionModifiers } from "./databaseGridModel";

interface DatabaseColumnHeaderParams {
  columnIndex: number;
  columnType: string;
  isSelected: () => boolean;
  onSelect: (columnIndex: number, modifiers: DatabaseGridSelectionModifiers) => void;
}
type DatabaseColumnHeaderProps = CustomHeaderProps<DatabaseGridRow> & DatabaseColumnHeaderParams;

function selectionModifiers(event: ReactMouseEvent): DatabaseGridSelectionModifiers {
  return {
    additive: event.ctrlKey || event.metaKey,
    extend: event.shiftKey,
  };
}
export function DatabaseColumnHeader({
  columnIndex,
  columnType,
  displayName,
  isSelected,
  onSelect,
}: DatabaseColumnHeaderProps) {
  const selected = isSelected();

  return (
    <div
      className={[
        "group flex h-full min-w-0 flex-1 cursor-default items-center gap-1",
        selected ? "text-primary" : "",
      ].join(" ")}
      onClick={(event) => onSelect(columnIndex, selectionModifiers(event))}
      title={`${displayName} (${columnType})`}
    >
      <span className="flex min-w-0 flex-1 flex-col justify-center leading-none">
        <span className="truncate text-xs font-semibold leading-4">{displayName}</span>
        <span className="truncate text-[10px] font-normal leading-3 text-muted-foreground">
          {columnType}
        </span>
      </span>
    </div>
  );
}

export function DatabaseCellRenderer({ value }: CustomCellRendererProps<DatabaseGridRow, unknown>) {
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

  const displayValue = value === null || value === undefined ? "null" : String(value);
  return (
    <span
      className={[
        "block w-full truncate",
        typeof value === "number" ? "text-right tabular-nums" : "",
        value === null || value === undefined ? "text-muted-foreground" : "",
      ].join(" ")}
    >
      {displayValue}
    </span>
  );
}

interface DatabaseRowMarkerParams {
  onSelectRow: (rowIndex: number, modifiers: DatabaseGridSelectionModifiers) => void;
}

type DatabaseRowMarkerProps = CustomCellRendererProps<DatabaseGridRow, number> &
  DatabaseRowMarkerParams;

export function DatabaseRowMarker({ data, node, onSelectRow, value }: DatabaseRowMarkerProps) {
  const selected = node.isSelected() === true;

  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={String(value ?? "")}
      aria-pressed={selected}
      className="group flex h-full w-full items-center justify-end gap-1.5 px-2 text-[11px] tabular-nums text-muted-foreground"
      onMouseDown={(event) => {
        if (event.button !== 0 || !data) return;
        event.preventDefault();
        event.stopPropagation();
        onSelectRow(data.sourceRowIndex, selectionModifiers(event));
      }}
    >
      <span
        aria-hidden="true"
        className={[
          "size-3.5 items-center justify-center rounded-[3px] border text-[10px] leading-none",
          selected
            ? "flex border-primary bg-primary text-primary-foreground"
            : "hidden border-muted-foreground/60 bg-transparent group-hover:flex",
        ].join(" ")}
      >
        {selected ? "✓" : null}
      </span>
      <span className={selected ? "" : "group-hover:hidden"}>{value}</span>
    </button>
  );
}

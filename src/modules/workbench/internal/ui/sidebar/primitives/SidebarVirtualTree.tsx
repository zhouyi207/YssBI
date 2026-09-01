import { useCallback, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Empty, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";

export interface SidebarVirtualTreeProps<Row> {
  rows: readonly Row[];
  ariaLabel: string;
  emptyMessage: string;
  getRowKey: (row: Row, index: number) => string | number;
  getRowDepth: (row: Row) => number;
  estimateSize: (row: Row) => number;
  renderRow: (row: Row) => React.ReactNode;
}

export function SidebarVirtualTree<Row>({
  rows,
  ariaLabel,
  emptyMessage,
  getRowKey,
  getRowDepth,
  estimateSize,
  renderRow,
}: SidebarVirtualTreeProps<Row>) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  const pendingFocusIndexRef = useRef<number | null>(null);
  const [tabStopIndex, setTabStopIndex] = useState(0);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => viewportRef.current,
    getItemKey: (index) => {
      const row = rows[index];
      return row ? getRowKey(row, index) : index;
    },
    estimateSize: (index) => {
      const row = rows[index];
      return row ? estimateSize(row) : 0;
    },
    overscan: 8,
    useFlushSync: false,
  });
  const virtualRows = virtualizer.getVirtualItems();
  const renderedTabStopIndex = virtualRows.some(({ index }) => index === tabStopIndex)
    ? tabStopIndex
    : (virtualRows[0]?.index ?? -1);

  const measureAndFocusRow = useCallback(
    (element: HTMLDivElement | null) => {
      virtualizer.measureElement(element);
      if (!element) return;

      const index = Number(element.dataset.sidebarTreeRowIndex);
      if (pendingFocusIndexRef.current !== index) return;
      pendingFocusIndexRef.current = null;
      element.focus({ preventScroll: true });
    },
    [virtualizer],
  );

  const focusRow = useCallback(
    (index: number) => {
      if (rows.length === 0) return;
      const nextIndex = Math.max(0, Math.min(index, rows.length - 1));
      pendingFocusIndexRef.current = nextIndex;
      setTabStopIndex(nextIndex);
      virtualizer.scrollToIndex(nextIndex, { align: "auto" });

      const rowElement = treeRef.current?.querySelector<HTMLElement>(
        `[data-sidebar-tree-row-index="${nextIndex}"]`,
      );
      if (!rowElement) return;
      pendingFocusIndexRef.current = null;
      rowElement.focus({ preventScroll: true });
    },
    [rows.length, virtualizer],
  );

  const handleRowKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>, rowIndex: number) => {
      if (event.target !== event.currentTarget || event.altKey || event.ctrlKey || event.metaKey) {
        return;
      }

      let nextIndex: number;
      switch (event.key) {
        case "ArrowDown":
          nextIndex = rowIndex + 1;
          break;
        case "ArrowUp":
          nextIndex = rowIndex - 1;
          break;
        case "Home":
          nextIndex = 0;
          break;
        case "End":
          nextIndex = rows.length - 1;
          break;
        default:
          return;
      }

      event.preventDefault();
      event.stopPropagation();
      focusRow(nextIndex);
    },
    [focusRow, rows.length],
  );

  return (
    <ScrollArea viewportRef={viewportRef} orientation="vertical" className="min-h-0 min-w-0 flex-1">
      {rows.length === 0 ? (
        <Empty className="gap-1 rounded-none px-2 py-4">
          <EmptyHeader>
            <EmptyTitle className="text-xs font-normal text-muted-foreground">
              {emptyMessage}
            </EmptyTitle>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="px-1 py-1">
          <div
            ref={treeRef}
            role="tree"
            aria-label={ariaLabel}
            className="relative w-full"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {virtualRows.map((virtualRow) => {
              const row = rows[virtualRow.index];
              if (!row) return null;
              return (
                <div
                  key={virtualRow.key}
                  ref={measureAndFocusRow}
                  data-index={virtualRow.index}
                  data-sidebar-tree-row-index={virtualRow.index}
                  className="absolute left-0 top-0 w-full"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                  role="treeitem"
                  aria-level={getRowDepth(row) + 1}
                  tabIndex={virtualRow.index === renderedTabStopIndex ? 0 : -1}
                  onFocus={() => setTabStopIndex(virtualRow.index)}
                  onKeyDown={(event) => handleRowKeyDown(event, virtualRow.index)}
                >
                  {renderRow(row)}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </ScrollArea>
  );
}

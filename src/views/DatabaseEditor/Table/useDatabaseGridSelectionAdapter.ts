import {
  useCallback,
  useEffect,
  useRef,
  type RefObject,
} from 'react';
import type {
  CellFocusedEvent,
  CellMouseDownEvent,
  CellMouseOverEvent,
  GridApi,
  GridReadyEvent,
  SelectionChangedEvent,
} from 'ag-grid-community';
import type { AgGridReact } from 'ag-grid-react';
import type { DatabaseGridSelection } from '@/features/application/databaseEditor';
import {
  createCellRange,
  createKeyboardCellSelection,
  dataColumnIndexFromId,
  updateIndexSelection,
  type DatabaseGridCellAddress,
  type DatabaseGridCellRange,
  type DatabaseGridRow,
  type DatabaseGridSelectionModifiers,
} from './databaseGridModel';

interface CellDragState {
  anchor: DatabaseGridCellAddress;
  retainedRanges: readonly DatabaseGridCellRange[];
  lastCell: DatabaseGridCellAddress;
}

interface UseDatabaseGridSelectionAdapterParams {
  gridRef: RefObject<AgGridReact<DatabaseGridRow> | null>;
  selection: DatabaseGridSelection | null;
  rowCount: number;
  columnCount: number;
  rowData: readonly DatabaseGridRow[];
  onSelectionChange: (selection: DatabaseGridSelection | null) => void;
}

function browserMouseEvent(event: Event | null | undefined): MouseEvent | null {
  return event instanceof MouseEvent ? event : null;
}

function selectionModifiers(event: MouseEvent): DatabaseGridSelectionModifiers {
  return {
    additive: event.ctrlKey || event.metaKey,
    extend: event.shiftKey,
  };
}

function sameIndices(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function useDatabaseGridSelectionAdapter({
  gridRef,
  selection,
  rowCount,
  columnCount,
  rowData,
  onSelectionChange,
}: UseDatabaseGridSelectionAdapterParams) {
  const selectionRef = useRef(selection);
  const rowCountRef = useRef(rowCount);
  const columnCountRef = useRef(columnCount);
  const rowSelectionAnchorRef = useRef<number | null>(null);
  const columnSelectionAnchorRef = useRef<number | null>(null);
  const cellSelectionAnchorRef = useRef<DatabaseGridCellAddress | null>(null);
  const pointerFocusedCellRef = useRef<DatabaseGridCellAddress | null>(null);
  const keyboardExtendRef = useRef<boolean | null>(null);
  const cellDragRef = useRef<CellDragState | null>(null);
  const syncingRowSelectionRef = useRef(false);

  selectionRef.current = selection;
  rowCountRef.current = rowCount;
  columnCountRef.current = columnCount;

  const publishCellSelection = useCallback((
    activeCell: DatabaseGridCellAddress,
    anchor: DatabaseGridCellAddress,
    retainedRanges: readonly DatabaseGridCellRange[],
  ) => {
    onSelectionChange({
      type: 'cells',
      activeCell,
      ranges: [...retainedRanges, createCellRange(anchor, activeCell)],
    });
  }, [onSelectionChange]);

  const handleRowSelect = useCallback((
    rowIndex: number,
    modifiers: DatabaseGridSelectionModifiers,
  ) => {
    const current = selectionRef.current;
    const currentRows = current?.type === 'rows' ? current.rows : [];
    const nextRows = updateIndexSelection(
      currentRows,
      rowIndex,
      rowSelectionAnchorRef.current,
      modifiers,
      rowCountRef.current,
    );
    if (!modifiers.extend) rowSelectionAnchorRef.current = rowIndex;
    onSelectionChange(nextRows.length > 0 ? { type: 'rows', rows: nextRows } : null);
  }, [onSelectionChange]);

  const handleColumnSelect = useCallback((
    columnIndex: number,
    modifiers: DatabaseGridSelectionModifiers,
  ) => {
    const current = selectionRef.current;
    const currentColumns = current?.type === 'columns' ? current.columns : [];
    const nextColumns = updateIndexSelection(
      currentColumns,
      columnIndex,
      columnSelectionAnchorRef.current,
      modifiers,
      columnCountRef.current,
    );
    if (!modifiers.extend) columnSelectionAnchorRef.current = columnIndex;
    onSelectionChange(nextColumns.length > 0
      ? { type: 'columns', columns: nextColumns }
      : null);
  }, [onSelectionChange]);

  const applyControlledSelection = useCallback((api: GridApi<DatabaseGridRow>) => {
    const current = selectionRef.current;
    const selectedRows = new Set(current?.type === 'rows' ? current.rows : []);
    syncingRowSelectionRef.current = true;
    try {
      api.forEachNode((node) => {
        const rowIndex = node.data?.sourceRowIndex;
        const shouldSelect = rowIndex !== undefined && selectedRows.has(rowIndex);
        if (node.isSelected() !== shouldSelect) node.setSelected(shouldSelect, false, 'api');
      });
    } finally {
      syncingRowSelectionRef.current = false;
    }
    api.refreshCells({ force: true });
    api.refreshHeader();
  }, []);

  useEffect(() => {
    if (!selection) {
      rowSelectionAnchorRef.current = null;
      columnSelectionAnchorRef.current = null;
      cellSelectionAnchorRef.current = null;
      pointerFocusedCellRef.current = null;
      keyboardExtendRef.current = null;
      cellDragRef.current = null;
    }
    const api = gridRef.current?.api;
    if (api) applyControlledSelection(api);
  }, [selection, rowData, gridRef, applyControlledSelection]);

  const handleGridReady = useCallback((event: GridReadyEvent<DatabaseGridRow>) => {
    applyControlledSelection(event.api);
  }, [applyControlledSelection]);

  const handleSelectionChanged = useCallback((event: SelectionChangedEvent<DatabaseGridRow>) => {
    if (syncingRowSelectionRef.current
      || event.source === 'api'
      || event.source === 'rowDataChanged'
      || event.source === 'gridInitializing') {
      return;
    }
    const selectedRows = (event.selectedNodes ?? [])
      .map((node) => node.data?.sourceRowIndex)
      .filter((rowIndex): rowIndex is number => rowIndex !== undefined)
      .sort((left, right) => left - right);
    const current = selectionRef.current;
    if (current?.type === 'rows' && sameIndices(current.rows, selectedRows)) return;
    onSelectionChange(selectedRows.length > 0 ? { type: 'rows', rows: selectedRows } : null);
  }, [onSelectionChange]);

  const handleCellMouseDown = useCallback((event: CellMouseDownEvent<DatabaseGridRow>) => {
    const mouseEvent = browserMouseEvent(event.event);
    const columnIndex = dataColumnIndexFromId(event.column.getColId());
    if (!mouseEvent || mouseEvent.button !== 0 || columnIndex === null || !event.data) return;

    const target = { row: event.data.sourceRowIndex, column: columnIndex };
    const modifiers = selectionModifiers(mouseEvent);
    const current = selectionRef.current;
    const retainedRanges = modifiers.additive && current?.type === 'cells'
      ? current.ranges
      : [];
    const anchor = modifiers.extend
      ? cellSelectionAnchorRef.current ?? (current?.type === 'cells' ? current.activeCell : target)
      : target;
    if (!modifiers.extend) cellSelectionAnchorRef.current = target;
    pointerFocusedCellRef.current = target;
    keyboardExtendRef.current = null;
    cellDragRef.current = { anchor, retainedRanges, lastCell: target };
    publishCellSelection(target, anchor, retainedRanges);
  }, [publishCellSelection]);

  const handleCellMouseOver = useCallback((event: CellMouseOverEvent<DatabaseGridRow>) => {
    const drag = cellDragRef.current;
    const mouseEvent = browserMouseEvent(event.event);
    const columnIndex = dataColumnIndexFromId(event.column.getColId());
    if (!drag || !mouseEvent || (mouseEvent.buttons & 1) === 0
      || columnIndex === null || !event.data) {
      return;
    }
    const target = { row: event.data.sourceRowIndex, column: columnIndex };
    if (target.row === drag.lastCell.row && target.column === drag.lastCell.column) return;
    drag.lastCell = target;
    publishCellSelection(target, drag.anchor, drag.retainedRanges);
  }, [publishCellSelection]);

  const captureCellKeyboard = useCallback((event: KeyboardEvent) => {
    keyboardExtendRef.current = event.key.startsWith('Arrow') ? event.shiftKey : null;
  }, []);

  const handleCellFocused = useCallback((event: CellFocusedEvent<DatabaseGridRow>) => {
    if (event.rowIndex === null || !event.column) return;
    const columnId = typeof event.column === 'string'
      ? event.column
      : event.column.getColId();
    const columnIndex = dataColumnIndexFromId(columnId);
    if (columnIndex === null || event.rowIndex < 0 || event.rowIndex >= rowCountRef.current) return;

    const target = { row: event.rowIndex, column: columnIndex };
    const pointerCell = pointerFocusedCellRef.current;
    const extend = keyboardExtendRef.current === true;
    pointerFocusedCellRef.current = null;
    keyboardExtendRef.current = null;
    if (pointerCell?.row === target.row && pointerCell.column === target.column) return;
    if (event.sourceEvent instanceof MouseEvent) return;

    const next = createKeyboardCellSelection(
      selectionRef.current,
      cellSelectionAnchorRef.current,
      target,
      extend,
    );
    cellSelectionAnchorRef.current = next.anchor;
    onSelectionChange(next.selection);
  }, [onSelectionChange]);

  return {
    selectionRef,
    captureCellKeyboard,
    handleCellFocused,
    handleCellMouseDown,
    handleCellMouseOver,
    handleColumnSelect,
    handleGridReady,
    handleRowSelect,
    handleSelectionChanged,
  };
}

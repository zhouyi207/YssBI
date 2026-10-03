import { useEffect, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import type { ColumnInfo } from "@/shared/types/domain/database";
import { useDatabaseRead } from "@/features/core/database/read";
import { useResourceRead } from "@/features/core/resource/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import {
  useDataLoader,
  useSelection,
  useDatabaseEditorKeyboard,
  getGridSelectionPrimaryCellText,
  useDatabaseExport,
} from "@/features/application/databaseEditor";
import { useEditorPaneStateStore } from "@/modules/workbench/public";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { DataTable } from "./Table";
import { Toolbar } from "./Layout";

const EMPTY_COLUMNS: readonly ColumnInfo[] = [];

interface DatabaseEditorContentProps {
  databaseId: string;
  panelInstanceId: string;
}

export function DatabaseEditorContent({ databaseId, panelInstanceId }: DatabaseEditorContentProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { columns, rowCount, columnCount } = useDatabaseRead(
    useShallow((snapshot) => {
      const database = snapshot.databases[databaseId];
      return {
        columns: database?.columns ?? EMPTY_COLUMNS,
        rowCount: database?.rowCount ?? 0,
        columnCount: database?.columnCount ?? 0,
      };
    }),
  );
  const revision = useResourceRead(
    (snapshot) => snapshot.resources[resourceKey({ kind: "database", id: databaseId })]?.revision,
  );
  const dataLoader = useDataLoader(databaseId);
  const exportDatabase = useDatabaseExport(databaseId);
  const selection = useSelection({
    columnCount: columns.length,
    rowCount: dataLoader.loadedRows.length,
  });
  const { loadInitialRows } = dataLoader;
  const { clearSelection } = selection;

  useDatabaseEditorKeyboard({ containerRef, selectAll: selection.selectAll, clearSelection });

  useEffect(() => {
    void loadInitialRows(databaseId).catch((error) =>
      reportViewIssue("app", error, "DatabaseEditor"),
    );
    clearSelection();
  }, [databaseId, revision, loadInitialRows, clearSelection]);

  useEffect(() => {
    clearSelection();
  }, [dataLoader.pageIndex, columns.length, clearSelection]);

  const selectedCellText = useMemo(
    () =>
      getGridSelectionPrimaryCellText(
        selection.selection,
        columns.length,
        dataLoader.loadedRows.length,
        dataLoader.loadedRows,
      ),
    [selection.selection, columns.length, dataLoader.loadedRows],
  );

  useEffect(() => {
    useEditorPaneStateStore.getState().setDatabaseView(panelInstanceId, {
      databaseId,
      selectedCellText,
    });
  }, [panelInstanceId, databaseId, selectedCellText]);

  useEffect(
    () => () => useEditorPaneStateStore.getState().setDatabaseView(panelInstanceId, undefined),
    [panelInstanceId],
  );

  return (
    <div
      ref={containerRef}
      data-database-editor={databaseId}
      className="@container flex h-full min-h-0 w-full flex-col overflow-hidden bg-background text-foreground font-sans"
    >
      <div className="flex min-h-0 flex-1 overflow-hidden bg-muted/30">
        <DataTable
          columns={columns}
          loadedRows={dataLoader.loadedRows}
          loadedRowIds={dataLoader.loadedRowIds}
          pageStartIndex={dataLoader.pageStartIndex}
          loading={dataLoader.loading}
          selection={selection.selection}
          onSelectionChange={selection.setSelection}
        />
      </div>
      <Toolbar
        loading={dataLoader.loading}
        totalRowCount={rowCount}
        columnCount={columnCount}
        pageIndex={dataLoader.pageIndex}
        pageSize={dataLoader.pageSize}
        totalPages={dataLoader.totalPages}
        lastFetchMs={dataLoader.lastFetchMs}
        onPreviousPage={dataLoader.goToPreviousPage}
        onNextPage={dataLoader.goToNextPage}
        onRefresh={dataLoader.reloadAllData}
        onExport={exportDatabase}
      />
    </div>
  );
}

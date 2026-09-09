import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { useDatabaseRead } from "@/features/core/database/read";
import {
  useDataLoader,
  useSelection,
  useDatabaseEditorKeyboard,
  getGridSelectionPrimaryCellText,
  useDatabaseExport,
} from "@/features/application/databaseEditor";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { DataTable } from "./Table";
import { Toolbar } from "./Layout";

interface DatabaseEditorContentProps {
  databaseId: string | null;
  renderHeader?: (selectedCellText: string) => ReactNode;
  refreshProjectOnRefresh?: boolean;
}

export function DatabaseEditorContent({
  databaseId,
  renderHeader,
  refreshProjectOnRefresh = false,
}: DatabaseEditorContentProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const database = useDatabaseRead((snapshot) =>
    databaseId ? snapshot.databases[databaseId] : null,
  );
  const columns = database?.columns ?? [];
  const dataLoader = useDataLoader(databaseId);
  const exportDatabase = useDatabaseExport(databaseId);
  const selection = useSelection({
    columnCount: columns.length,
    rowCount: dataLoader.loadedRows.length,
  });
  const { loadInitialRows, setLoadedRows } = dataLoader;
  const { clearSelection } = selection;

  useDatabaseEditorKeyboard({ containerRef, selectAll: selection.selectAll, clearSelection });

  useEffect(() => {
    if (databaseId) {
      void loadInitialRows(databaseId).catch((error) =>
        reportViewIssue("app", error, "DatabaseEditor"),
      );
    } else {
      setLoadedRows([]);
    }
    clearSelection();
  }, [databaseId, loadInitialRows, setLoadedRows, clearSelection]);

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

  return (
    <div
      ref={containerRef}
      data-database-editor={databaseId ?? undefined}
      className="flex h-full min-h-0 w-full flex-col overflow-hidden bg-background text-foreground font-sans"
    >
      {renderHeader?.(selectedCellText)}
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
        totalRowCount={database?.rowCount ?? 0}
        columnCount={database?.columnCount ?? 0}
        pageIndex={dataLoader.pageIndex}
        pageSize={dataLoader.pageSize}
        totalPages={dataLoader.totalPages}
        lastFetchMs={dataLoader.lastFetchMs}
        exportEnabled={Boolean(databaseId)}
        onPreviousPage={dataLoader.goToPreviousPage}
        onNextPage={dataLoader.goToNextPage}
        onRefresh={refreshProjectOnRefresh ? dataLoader.refreshData : dataLoader.reloadAllData}
        onExport={exportDatabase}
      />
    </div>
  );
}

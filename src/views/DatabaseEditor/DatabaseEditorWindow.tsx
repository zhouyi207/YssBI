import React, { useEffect, useState, useMemo, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DatabaseService } from '@/services/database/databaseService';
import { useProjectSync } from '@/features/application/initialization';
import { usePersistedWindow } from '@/features/application/window';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { useDataLoader, useSelection, useDatabaseEditorKeyboard, getGridSelectionPrimaryCellText, useDatabaseExport } from '@/features/application/databaseEditor';
import { TitleBar, Toolbar, type DataframeOption } from './Layout';
import { DataTable } from './Table';
import { logger } from '@/utils/appLogger';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

type DatabaseMetadataUpdater = (
  id: string,
  changes: { name: string; columns: Array<{ name: string; type: string }>; rowCount: number; columnCount: number },
) => void;

export async function hydrateDatabaseEditorMetadata(
  id: string,
  updateDatabase: DatabaseMetadataUpdater,
  isCancelled: () => boolean = () => false,
): Promise<void> {
  const identity = captureProjectIdentity();
  try {
    const meta = await DatabaseService.getDatabaseMeta(identity.projectInstanceId, id);
    if (isCancelled() || !isCurrentProjectIdentity(identity)) return;
    updateDatabase(id, {
      name: meta.name,
      columns: meta.columns,
      rowCount: meta.rowCount,
      columnCount: meta.columnCount,
    });
  } catch (error) {
    if (!isCancelled() && isCurrentProjectIdentity(identity)) {
      logger.data.warn('getDatabaseMeta failed: ' + String(error), 'DatabaseEditorWindow');
    }
  }
}

function getDatabaseIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('database');
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf('?');
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get('database');
}

export const DatabaseEditorWindow: React.FC = () => {
  const dataframes = useDatabaseStore(s => s.databases);

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const hasInitializedDfRef = useRef(false);

  usePersistedWindow('databaseEditor');

  useProjectSync();

  // Derived state
  const selectedDf = selectedDfId ? dataframes[selectedDfId] : null;
  const columns = ((selectedDf as { columns?: Array<{ name: string; type: string }> })?.columns ?? []);
  const totalRowCount = (selectedDf as { rowCount?: number })?.rowCount ?? 0;

  // Data loading
  const dataLoader = useDataLoader(selectedDfId);
  const exportDatabase = useDatabaseExport(selectedDfId);

  // Selection
  const sel = useSelection({
    columnCount: columns.length,
    rowCount: dataLoader.loadedRows.length,
  });

  // Keyboard shortcuts
  useDatabaseEditorKeyboard({
    selectAll: sel.selectAll,
    clearSelection: sel.clearSelection,
  });

  // 首次有数据时选中 URL 指定或第一个 DataFrame；之后仅在当前选中被删除时回退
  useEffect(() => {
    const ids = Object.keys(dataframes);
    if (ids.length === 0) {
      hasInitializedDfRef.current = false;
      setSelectedDfId(null);
      dataLoader.setLoadedRows([]);
      return;
    }
    const dbFromUrl = getDatabaseIdFromUrl();
    const preferred = dbFromUrl && dataframes[dbFromUrl] ? dbFromUrl : ids[0];

    if (!hasInitializedDfRef.current) {
      hasInitializedDfRef.current = true;
      setSelectedDfId(preferred);
      return;
    }

    if (selectedDfId && !dataframes[selectedDfId]) {
      setSelectedDfId(ids[0] ?? null);
    }
  }, [dataframes, selectedDfId]);

  // Load data when dataframe changes
  useEffect(() => {
    if (selectedDfId) {
      dataLoader.loadInitialRows(selectedDfId);
    } else {
      dataLoader.setLoadedRows([]);
    }
    sel.clearSelection();
  }, [selectedDfId]);

  /** 分页或列结构变化后，保留旧选择会与当前页/列错位 */
  useEffect(() => {
    sel.clearSelection();
  }, [dataLoader.pageIndex, columns.length, sel.clearSelection]);

  // Auto-fetch meta if missing
  useEffect(() => {
    if (!selectedDfId) return;
    const df = dataframes[selectedDfId];
    if (!df) return;
    if (df.name && (df.columns?.length ?? 0) > 0) return;
    let cancelled = false;
    void hydrateDatabaseEditorMetadata(
      selectedDfId,
      useDatabaseStore.getState().updateDatabase,
      () => cancelled,
    );
    return () => { cancelled = true; };
  }, [selectedDfId, dataframes]);

  // 子窗口独立 WebView：先从后端同步项目，再展示窗口
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await initProjectSync();
        if (cancelled) return;
        await getCurrentWindow().show();
      } catch (e) {
        logger.app.error(String(e), 'DatabaseEditorWindow');
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const dfOptions: DataframeOption[] = useMemo(() => Object.entries(dataframes).map(([id, df]) => {
    const d = df as { name?: string; engine?: { csv?: { path?: string }; parquet?: { path?: string }; duckDb?: { table?: string } } };
    let label = d.name;
    if (!label && d.engine?.csv?.path) { const p = d.engine.csv.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    if (!label && d.engine?.parquet?.path) { const p = d.engine.parquet.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    return { label: String(label ?? id), value: id };
  }), [dataframes]);

  const selectedCellPreview = useMemo(
    () => getGridSelectionPrimaryCellText(
      sel.selection,
      columns.length,
      dataLoader.loadedRows.length,
      dataLoader.loadedRows,
    ),
    [sel.selection, columns.length, dataLoader.loadedRows],
  );

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground font-sans">
      <TitleBar
        dataframes={dfOptions}
        selectedDataframeId={selectedDfId}
        onSelectDataframe={setSelectedDfId}
        selectedCellText={selectedCellPreview}
      />

      <div className="flex min-h-0 flex-1 overflow-hidden bg-muted/30">
        <DataTable
          columns={columns}
          loadedRows={dataLoader.loadedRows}
          loadedRowIds={dataLoader.loadedRowIds}
          pageStartIndex={dataLoader.pageStartIndex}
          loading={dataLoader.loading}
          selection={sel.selection}
          onSelectionChange={sel.setSelection}
        />
      </div>

      <Toolbar
        loading={dataLoader.loading}
        totalRowCount={totalRowCount}
        columnCount={(selectedDf as { columnCount?: number })?.columnCount ?? 0}
        pageIndex={dataLoader.pageIndex}
        pageSize={dataLoader.pageSize}
        totalPages={dataLoader.totalPages}
        lastFetchMs={dataLoader.lastFetchMs}
        exportEnabled={Boolean(selectedDfId)}
        onPreviousPage={dataLoader.goToPreviousPage}
        onNextPage={dataLoader.goToNextPage}
        onRefresh={dataLoader.refreshData}
        onExport={exportDatabase}
      />

    </div>
  );
};

import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscPreview } from 'react-icons/vsc';
import { DatabaseService } from '@/services/database/databaseService';
import { useProjectSync } from '@/features/application/initialization';
import { usePersistedWindow, useWindowMaximized } from '@/features/application/window';
import { useDatabaseStore, initProjectSync } from '@/features/core/dataStore';
import { useDataLoader, useEditActions, useSelection, useDataViewKeyboard, getGridSelectionPrimaryCellText } from '@/features/application/dataView';
import { TitleBar, Toolbar, type DataframeOption } from './Layout';
import { DataTable } from './Table';
import { TableContextMenu } from './ContextMenu';
import type { ContextMenuState } from './ContextMenu';
import { logger } from '@/utils/appLogger';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import { SourceService, UnifiedDataView, type SourceDescriptor } from '@/features/core/dataView';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';

function getDatabaseIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('database');
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf('?');
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get('database');
}

function getSourceIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('sourceId');
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf('?');
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get('sourceId');
}

export const DataViewWindow: React.FC = () => {
  const sourceId = useMemo(() => getSourceIdFromUrl(), []);
  const [sourceDescriptor, setSourceDescriptor] = useState<SourceDescriptor | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const isSourceWindowMaximized = useWindowMaximized('SourceDataViewWindow');
  const dataframes = useDatabaseStore(s => s.databases);

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const hasInitializedDfRef = useRef(false);

  usePersistedWindow('dataView');

  useProjectSync();

  useEffect(() => {
    if (!sourceId) return;
    let cancelled = false;
    (async () => {
      try {
        const descriptor = await SourceService.getDescriptor(sourceId);
        if (cancelled) return;
        if (!descriptor) {
          setSourceError('No data source found');
          return;
        }
        setSourceDescriptor(descriptor);
        await getCurrentWindow().setTitle(descriptor.title).catch(() => {});
        await getCurrentWindow().show().catch(() => {});
      } catch (e) {
        if (!cancelled) setSourceError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => { cancelled = true; };
  }, [sourceId]);

  // Derived state
  const selectedDf = selectedDfId ? dataframes[selectedDfId] : null;
  const columns = ((selectedDf as { columns?: Array<{ name: string; type: string }> })?.columns ?? []);
  const totalRowCount = (selectedDf as { rowCount?: number })?.rowCount ?? 0;

  // Data loading
  const dataLoader = useDataLoader(selectedDfId);
  const dismissContextMenu = useCallback(() => setContextMenu(null), []);

  // Edit actions
  const edit = useEditActions({
    selectedDfId,
    columns,
    loadedRows: dataLoader.loadedRows,
    loadedRowIds: dataLoader.loadedRowIds,
    rowOffset: dataLoader.pageStartIndex,
    reloadAllData: dataLoader.reloadAllData,
  });

  // Selection
  const sel = useSelection({
    columnCount: columns.length,
    rowCount: dataLoader.loadedRows.length,
  });

  // Keyboard shortcuts
  useDataViewKeyboard({
    handleUndo: edit.handleUndo,
    handleRedo: edit.handleRedo,
    handleDeleteRow: edit.handleDeleteRow,
    selectAll: sel.selectAll,
    clearSelection: sel.clearSelection,
    dismissContextMenu,
    selection: sel.selection,
    selectedRowIndices: sel.selectedRowIndices,
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
    const df = dataframes[selectedDfId] as Record<string, unknown> | undefined;
    if (!df) return;
    if (df.name && Array.isArray(df.columns) && df.columns.length > 0) return;
    let cancelled = false;
    const id = selectedDfId;
    DatabaseService.getDatabaseMeta(selectedDfId)
      .then((meta) => {
        if (cancelled || id !== selectedDfId) return;
        useDatabaseStore.getState().updateDatabase(id, {
          name: meta.name, columns: meta.columns,
          rowCount: meta.rowCount, columnCount: meta.columnCount,
        });
      })
      .catch((e) => logger.data.warn('getDatabaseMeta failed: ' + String(e), 'DataViewWindow'));
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
        logger.app.error(String(e), 'DataViewWindow');
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // Dismiss context menu on any click
  useEffect(() => {
    const dismiss = () => setContextMenu(null);
    return addGlobalEventListener(window, 'click', dismiss);
  }, []);

  // Context menu handler
  const handleContextMenu = useCallback((position: { x: number; y: number }, target: { type: 'cell' | 'header' | 'row'; rowIndex?: number; colIndex?: number; colName?: string }) => {
    setContextMenu({ x: position.x, y: position.y, ...target });
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

  if (sourceId) {
    return (
      <div className="flex h-screen w-full flex-col overflow-hidden bg-[var(--workbench-bg)] text-[var(--workbench-fg)] font-sans">
        <WindowTitleBar childWindow>
          <div className="flex min-w-0 flex-1 items-center gap-2 px-3" data-tauri-drag-region>
            <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
              <VscPreview size={14} />
            </span>
            <span className="min-w-0 truncate text-[13px] font-semibold tracking-tight text-foreground">
              {sourceDescriptor?.title ?? 'Data View'}
            </span>
          </div>
          <WindowTitleBarActions>
            <WindowChromeControls isMaximized={isSourceWindowMaximized} />
          </WindowTitleBarActions>
        </WindowTitleBar>
        <div className="min-h-0 flex-1">
          {sourceError ? (
            <div className="flex h-full flex-1 items-center justify-center text-sm text-destructive">
              {sourceError}
            </div>
          ) : sourceDescriptor ? (
            <UnifiedDataView payload={sourceDescriptor} />
          ) : (
            <div className="flex h-full flex-1 items-center justify-center text-sm text-muted-foreground">
              Loading…
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-[var(--workbench-bg)] text-[var(--workbench-fg)] font-sans">
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
          pageStartIndex={dataLoader.pageStartIndex}
          loading={dataLoader.loading}
          selection={sel.selection}
          onSelectionChange={sel.setSelection}
          onCommitCellValue={edit.commitCellValue}
          onContextMenu={handleContextMenu}
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
        currentEditState={edit.currentEditState}
        onPreviousPage={dataLoader.goToPreviousPage}
        onNextPage={dataLoader.goToNextPage}
        onRefresh={dataLoader.refreshData}
        onSave={edit.handleSave}
        onUndo={edit.handleUndo}
        onRedo={edit.handleRedo}
        onExport={edit.handleExport}
      />

      {contextMenu && (
        <TableContextMenu
          menu={contextMenu}
          selectedRowIndices={sel.selectedRowIndices()}
          onAddRow={edit.handleAddRow}
          onDeleteRow={edit.handleDeleteRow}
          onRenameColumn={edit.handleRenameColumn}
          onAddColumn={edit.handleAddColumn}
          onDeleteColumn={edit.handleDeleteColumn}
          onClearSelection={sel.clearSelection}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
};

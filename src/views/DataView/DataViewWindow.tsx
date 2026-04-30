import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DatabaseService } from '@/services/database/databaseService';
import { useProjectSync } from '@/features/application/initialization';
import { useDatabaseStore, useEditStateStore } from '@/features/core/dataStore';
import { useDataLoader, useEditActions, useSelection, useDataViewKeyboard } from '@/features/application/dataView';
import { DataTabs, TitleBar, Toolbar, type DataframeOption } from './Layout';
import { DataTable } from './Table';
import { TableContextMenu } from './ContextMenu';
import type { ContextMenuState } from './ContextMenu';
import { logger } from '@/utils/appLogger';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

function getDatabaseIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('database');
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf('?');
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get('database');
}

export const DataViewWindow: React.FC = () => {
  const dataframes = useDatabaseStore(s => s.databases);
  const editStateByDatabase = useEditStateStore(s => s.editStateByDatabase);

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [openTabIds, setOpenTabIds] = useState<string[]>([]);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const hasInitializedTabsRef = useRef(false);

  useProjectSync();

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

  // Auto-open the requested dataframe, or the first available dataframe.
  useEffect(() => {
    const ids = Object.keys(dataframes);
    if (ids.length === 0) {
      hasInitializedTabsRef.current = false;
      setOpenTabIds([]);
      setSelectedDfId(null);
      dataLoader.setLoadedRows([]);
      return;
    }
    const dbFromUrl = getDatabaseIdFromUrl();
    const preferred = dbFromUrl && dataframes[dbFromUrl] ? dbFromUrl : ids[0];

    if (!hasInitializedTabsRef.current) {
      hasInitializedTabsRef.current = true;
      setOpenTabIds([preferred]);
      setSelectedDfId(preferred);
      return;
    }

    const existing = openTabIds.filter((id) => dataframes[id]);
    if (existing.length !== openTabIds.length) {
      setOpenTabIds(existing);
    }
    if (selectedDfId && !dataframes[selectedDfId]) {
      setSelectedDfId(existing[0] ?? null);
    }
  }, [dataframes, openTabIds, selectedDfId]);

  // Load data when selection changes
  useEffect(() => {
    if (selectedDfId) {
      dataLoader.loadInitialRows(selectedDfId);
    } else {
      dataLoader.setLoadedRows([]);
    }
    sel.clearSelection();
  }, [selectedDfId]);

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

  // Show window on mount
  useEffect(() => {
    getCurrentWindow().show().catch((e) => logger.app.error(String(e), 'DataViewWindow'));
    dataLoader.refreshData();
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

  // Toolbar options
  const dfOptions: DataframeOption[] = useMemo(() => Object.entries(dataframes).map(([id, df]) => {
    const d = df as { name?: string; engine?: { csv?: { path?: string }; parquet?: { path?: string } } };
    let label = d.name;
    if (!label && d.engine?.csv?.path) { const p = d.engine.csv.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    if (!label && d.engine?.parquet?.path) { const p = d.engine.parquet.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    return { label: String(label ?? id), value: id };
  }), [dataframes]);

  const labelByDfId = useMemo(() => new Map(dfOptions.map((option) => [option.value, option.label])), [dfOptions]);
  const tabs = useMemo(() => openTabIds.map((id) => ({
    id,
    label: labelByDfId.get(id) ?? id,
    isModified: Boolean(editStateByDatabase[id]?.isModified),
  })), [editStateByDatabase, labelByDfId, openTabIds]);

  const handleAddTab = useCallback((id: string) => {
    if (!dataframes[id]) return;
    setOpenTabIds((prev) => prev.includes(id) ? prev : [...prev, id]);
    setSelectedDfId(id);
  }, [dataframes]);

  const handleCloseTab = useCallback((id: string) => {
    setOpenTabIds((prev) => {
      const index = prev.indexOf(id);
      const next = prev.filter((tabId) => tabId !== id);
      if (selectedDfId === id) {
        const fallback = next[Math.min(index, next.length - 1)] ?? next[0] ?? null;
        setSelectedDfId(fallback);
      }
      return next;
    });
  }, [selectedDfId]);

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-muted/30 text-[var(--workbench-fg)] font-sans">
      <TitleBar isModified={edit.currentEditState.isModified} />

      <DataTabs
        tabs={tabs}
        options={dfOptions}
        activeTabId={selectedDfId}
        onSelectTab={setSelectedDfId}
        onAddTab={handleAddTab}
        onCloseTab={handleCloseTab}
      />

      <div className="flex min-h-0 flex-1 overflow-hidden">
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
        hasSelection={!!selectedDf}
        currentEditState={edit.currentEditState}
        onPreviousPage={dataLoader.goToPreviousPage}
        onNextPage={dataLoader.goToNextPage}
        onRefresh={dataLoader.refreshData}
        onSave={edit.handleSave}
        onUndo={edit.handleUndo}
        onRedo={edit.handleRedo}
        onReset={edit.handleReset}
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

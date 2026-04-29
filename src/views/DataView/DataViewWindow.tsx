import React, { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DatabaseService } from '@/services/database/databaseService';
import { useProjectSync } from '@/features/application/initialization';
import { useDatabaseStore, useColumnStatsStore, useColumnDistributionStore, useDatasetOverviewStore } from '@/features/core/dataStore';
import { useDataLoader, useEditActions, useSelection, useDataViewKeyboard } from '@/features/application/dataView';
import { DATA_VIEW_ROW_HEIGHT } from '@/app/appConfig/default';
import { TitleBar, Toolbar } from './Layout';
import { DataTable } from './Table';
import { TableContextMenu } from './ContextMenu';
import type { ContextMenuState } from './ContextMenu';
import { RightPanel } from './Stats';
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
  const { t } = useTranslation();
  const dataframes = useDatabaseStore(s => s.databases);
  const statsByDatabase = useColumnStatsStore(s => s.statsByDatabase);
  const distByDatabase = useColumnDistributionStore(s => s.distByDatabase);
  const overviewByDatabase = useDatasetOverviewStore(s => s.overviewByDatabase);

  const [selectedDfId, setSelectedDfId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [headerHeight, setHeaderHeight] = useState(0);

  useProjectSync();

  // Derived state
  const selectedDf = selectedDfId ? dataframes[selectedDfId] : null;
  const columns = ((selectedDf as { columns?: Array<{ name: string; type: string }> })?.columns ?? []);
  const totalRowCount = (selectedDf as { rowCount?: number })?.rowCount ?? 0;

  // Data loading
  const dataLoader = useDataLoader(selectedDfId);

  // Edit actions
  const edit = useEditActions({
    selectedDfId,
    columns,
    loadedRows: dataLoader.loadedRows,
    reloadAllData: dataLoader.reloadAllData,
    loadColumnStats: dataLoader.loadColumnStats,
  });

  // Selection
  const sel = useSelection({
    columnCount: columns.length,
    rowCount: dataLoader.loadedRows.length,
    isEditing: !!edit.editingCell,
  });

  // Keyboard shortcuts
  useDataViewKeyboard({
    handleUndo: edit.handleUndo,
    handleRedo: edit.handleRedo,
    cancelEdit: edit.cancelEdit,
    startEdit: edit.startEdit,
    handleDeleteRow: edit.handleDeleteRow,
    selectAll: sel.selectAll,
    clearSelection: sel.clearSelection,
    setSelection: sel.setSelection,
    dismissContextMenu: useCallback(() => setContextMenu(null), []),
    selection: sel.selection,
    activeCell: sel.activeCell,
    editingCell: edit.editingCell,
    selectedRowIndices: sel.selectedRowIndices,
    rowCount: dataLoader.loadedRows.length,
    columnCount: columns.length,
  });

  // Auto-select first dataframe, or database from URL (?database=id) when opened from sidebar eye icon
  useEffect(() => {
    const ids = Object.keys(dataframes);
    if (ids.length === 0) {
      setSelectedDfId(null);
      dataLoader.setLoadedRows([]);
      return;
    }
    const dbFromUrl = getDatabaseIdFromUrl();
    const preferred = dbFromUrl && dataframes[dbFromUrl] ? dbFromUrl : ids[0];
    if (!selectedDfId || !dataframes[selectedDfId]) setSelectedDfId(preferred);
  }, [dataframes, selectedDfId]);

  // Load data when selection changes
  useEffect(() => {
    if (selectedDfId) {
      dataLoader.loadInitialRows(selectedDfId);
      dataLoader.loadColumnStats(selectedDfId);
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

  // Scroll handler for infinite loading（固定总高度下：当可见行接近已加载末尾时触发加载）
  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const t = e.currentTarget;
    const lastVisibleRow = Math.floor((t.scrollTop + t.clientHeight) / DATA_VIEW_ROW_HEIGHT);
    if (lastVisibleRow >= dataLoader.loadedRows.length - 20) dataLoader.loadMoreRows();
  }, [dataLoader.loadMoreRows, dataLoader.loadedRows.length]);

  // Context menu handler
  const handleContextMenu = useCallback((e: React.MouseEvent, target: { type: 'cell' | 'header' | 'row'; rowIndex?: number; colIndex?: number; colName?: string }) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, ...target });
  }, []);

  // Toolbar options
  const dfOptions = Object.entries(dataframes).map(([id, df]) => {
    const d = df as { name?: string; engine?: { csv?: { path?: string }; parquet?: { path?: string } } };
    let label = d.name;
    if (!label && d.engine?.csv?.path) { const p = d.engine.csv.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    if (!label && d.engine?.parquet?.path) { const p = d.engine.parquet.path; label = p.replace(/^.*[/\\]/, '').replace(/\.[^.]+$/, '') || p; }
    return { label: String(label ?? id), value: id };
  });

  const columnStatsMap = selectedDfId ? statsByDatabase[selectedDfId] : undefined;
  const columnDistMap = selectedDfId ? distByDatabase[selectedDfId] : undefined;
  const currentOverview = selectedDfId ? overviewByDatabase[selectedDfId] : undefined;

  return (
    <div className="flex flex-col w-full h-screen bg-[var(--workbench-bg)] text-[var(--workbench-fg)] overflow-hidden font-sans">
      <TitleBar isModified={edit.currentEditState.isModified} />

      <Toolbar
        selectedDfId={selectedDfId}
        options={dfOptions.length > 0 ? dfOptions : [{ label: t('dataView.noDataFrame'), value: '' }]}
        loading={dataLoader.loading}
        totalRowCount={totalRowCount}
        columnCount={(selectedDf as { columnCount?: number })?.columnCount ?? 0}
        hasSelection={!!selectedDf}
        currentEditState={edit.currentEditState}
        onSelectDf={setSelectedDfId}
        onRefresh={dataLoader.refreshData}
        onUndo={edit.handleUndo}
        onRedo={edit.handleRedo}
        onReset={edit.handleReset}
        onExport={edit.handleExport}
      />

      <div className="flex-1 flex min-h-0 overflow-hidden">
        <DataTable
          columns={columns}
          loadedRows={dataLoader.loadedRows}
          totalRowCount={totalRowCount}
          loading={dataLoader.loading}
          loadingMore={dataLoader.loadingMore}
          scrollRef={dataLoader.scrollRef}
          onHeaderHeightChange={setHeaderHeight}
          selection={sel.selection}
          activeCell={sel.activeCell}
          editingCell={edit.editingCell}
          isInSelection={sel.isInSelection}
          onCellMouseDown={sel.handleCellMouseDown}
          onCellMouseEnter={sel.handleCellMouseEnter}
          onRowHeaderClick={sel.handleRowHeaderClick}
          onColHeaderClick={sel.handleColHeaderClick}
          onSelectAll={sel.selectAll}
          editValue={edit.editValue}
          editInputRef={edit.editInputRef}
          onEditValueChange={edit.setEditValue}
          onStartEdit={edit.startEdit}
          onCommitEdit={edit.commitEdit}
          onCancelEdit={edit.cancelEdit}
          onContextMenu={handleContextMenu}
          onCastColumn={edit.handleCastColumn}
          onScroll={handleScroll}
        />

        {selectedDf && columns.length > 0 && (
          <RightPanel
            columns={columns}
            overview={currentOverview}
            columnStatsMap={columnStatsMap}
            columnDistMap={columnDistMap}
            statsLoading={dataLoader.statsLoading}
            onCastColumn={edit.handleCastColumn}
          />
        )}
      </div>

      {contextMenu && (
        <TableContextMenu
          menu={contextMenu}
          selectedRowIndices={sel.selectedRowIndices()}
          onStartEdit={edit.startEdit}
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

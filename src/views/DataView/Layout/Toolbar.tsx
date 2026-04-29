import React from 'react';
import { useTranslation } from 'react-i18next';
import { VscRefresh, VscDiscard, VscExport } from 'react-icons/vsc';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { Select } from '@/shared/ui';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';

interface DataframeOption { label: string; value: string; }

interface ToolbarProps {
  selectedDfId: string | null;
  options: DataframeOption[];
  loading: boolean;
  totalRowCount: number;
  columnCount: number;
  hasSelection: boolean;
  currentEditState: EditState;
  onSelectDf: (id: string) => void;
  onRefresh: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onReset: () => void;
  onExport: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  selectedDfId, options, loading, totalRowCount, columnCount, hasSelection,
  currentEditState, onSelectDf, onRefresh, onUndo, onRedo, onReset, onExport,
}) => {
  const { t } = useTranslation();

  return (
  <div className="h-12 border-b border-border flex items-center px-4 gap-2 bg-[var(--sidebar-bg)] shrink-0">
    <div className="w-[240px]">
      <Select
        value={selectedDfId || ''}
        onChange={onSelectDf}
        options={options}
        disabled={options.length === 0 || (options.length === 1 && options[0]?.value === '')}
      />
    </div>

    <Button type="button" variant="ghost" size="icon-sm" onClick={onRefresh} title={t("common.refresh")}>
      <VscRefresh className={loading ? 'animate-spin' : ''} size={15} />
    </Button>

    <Separator orientation="vertical" className="mx-1 h-5" />

    <Button type="button" variant="ghost" size="icon-sm" onClick={onUndo} disabled={!currentEditState.canUndo} title={`${t("common.undo")} (Ctrl+Z)`}>
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 10h13a4 4 0 010 8H9" /><path d="M3 10l4-4M3 10l4 4" /></svg>
    </Button>
    <Button type="button" variant="ghost" size="icon-sm" onClick={onRedo} disabled={!currentEditState.canRedo} title={`${t("common.redo")} (Ctrl+Shift+Z)`}>
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 10H8a4 4 0 000 8h6" /><path d="M21 10l-4-4M21 10l-4 4" /></svg>
    </Button>

    <Button type="button" variant="ghost" size="icon-sm" onClick={onReset} disabled={!currentEditState.isModified} title={t("dataView.resetToOriginal")}>
      <VscDiscard size={15} />
    </Button>

    <Separator orientation="vertical" className="mx-1 h-5" />

    <Button type="button" variant="ghost" size="icon-sm" onClick={onExport} disabled={!hasSelection} title={t("common.export")}>
      <VscExport size={15} />
    </Button>

    {hasSelection && (
      <div className="ml-auto flex items-center gap-4 text-[10px] font-mono opacity-50">
        <span>{t("dataView.columns").toUpperCase()}: {columnCount}</span>
        <span>{t("dataView.rows").toUpperCase()}: {totalRowCount}</span>
      </div>
    )}
  </div>
  );
};

import React from 'react';
import { useTranslation } from 'react-i18next';
import { VscChevronLeft, VscChevronRight, VscRefresh, VscDiscard, VscExport, VscSave } from 'react-icons/vsc';
import type { EditState } from '@/features/core/dataStore/editStateStore';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';

interface ToolbarProps {
  loading: boolean;
  totalRowCount: number;
  columnCount: number;
  pageIndex: number;
  pageSize: number;
  totalPages: number;
  lastFetchMs: number | null;
  hasSelection: boolean;
  currentEditState: EditState;
  onPreviousPage: () => void;
  onNextPage: () => void;
  onRefresh: () => void;
  onSave: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onReset: () => void;
  onExport: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  loading, totalRowCount, columnCount, pageIndex, pageSize, totalPages, lastFetchMs, hasSelection,
  currentEditState, onPreviousPage, onNextPage, onRefresh, onSave, onUndo, onRedo, onReset, onExport,
}) => {
  const { t } = useTranslation();
  const pageStart = totalRowCount === 0 ? 0 : pageIndex * pageSize + 1;
  const pageEnd = Math.min(totalRowCount, (pageIndex + 1) * pageSize);
  const fetchTimeLabel = lastFetchMs === null ? '-' : lastFetchMs >= 1000 ? `${(lastFetchMs / 1000).toFixed(2)}s` : `${lastFetchMs}ms`;

  return (
  <div className="flex min-h-12 shrink-0 items-center gap-2 border-t border-border bg-card/90 px-3 py-2">
    <div className="flex shrink-0 items-center gap-1 rounded-md border border-border bg-background p-0.5 shadow-sm">
      <Button type="button" variant="ghost" size="icon-sm" onClick={onRefresh} title={t("common.refresh")}>
        <VscRefresh className={loading ? 'animate-spin' : ''} size={15} />
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" onClick={onSave} disabled={!currentEditState.isModified} title="保存">
        <VscSave size={15} />
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" onClick={onReset} disabled={!currentEditState.isModified} title={t("dataView.resetToOriginal")}>
        <VscDiscard size={15} />
      </Button>
      <Separator orientation="vertical" className="h-4" />
      <Button type="button" variant="ghost" size="icon-sm" onClick={onUndo} disabled={!currentEditState.canUndo} title={`${t("common.undo")} (Ctrl+Z)`}>
        <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 10h13a4 4 0 010 8H9" /><path d="M3 10l4-4M3 10l4 4" /></svg>
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" onClick={onRedo} disabled={!currentEditState.canRedo} title={`${t("common.redo")} (Ctrl+Shift+Z)`}>
        <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 10H8a4 4 0 000 8h6" /><path d="M21 10l-4-4M21 10l-4 4" /></svg>
      </Button>
      <Separator orientation="vertical" className="h-4" />
      <Button type="button" variant="ghost" size="icon-sm" onClick={onExport} disabled={!hasSelection} title={t("common.export")}>
        <VscExport size={15} />
      </Button>
    </div>

    <div className="flex min-w-0 flex-1 items-center justify-center">
    <div className="flex shrink-0 items-center overflow-hidden rounded-md border border-border bg-background text-card-foreground shadow-sm">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="rounded-none border-r border-border"
        onClick={onPreviousPage}
        disabled={loading || pageIndex <= 0}
        title="上一页"
      >
        <VscChevronLeft size={15} />
      </Button>

      <div className="flex h-6 min-w-[184px] items-center justify-center gap-1 px-2 text-[11px]">
        <span className="font-medium text-foreground">{pageStart}-{pageEnd}</span>
        <span className="text-muted-foreground">/</span>
        <span className="text-muted-foreground">{totalRowCount}</span>
        <span className="ml-1 rounded-sm bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
          {pageIndex + 1} / {totalPages}
        </span>
      </div>

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="rounded-none border-l border-border"
        onClick={onNextPage}
        disabled={loading || pageIndex >= totalPages - 1}
        title="下一页"
      >
        <VscChevronRight size={15} />
      </Button>
    </div>
    </div>

    <div className="hidden shrink-0 items-center gap-2 text-[10px] font-medium text-muted-foreground xl:flex">
      <div className="rounded-md border border-border bg-background px-2 py-1">
        {t("dataView.columns").toUpperCase()}: <span className="text-foreground">{columnCount}</span>
      </div>
      <div className="rounded-md border border-border bg-background px-2 py-1">
        {t("dataView.rows").toUpperCase()}: <span className="text-foreground">{totalRowCount}</span>
      </div>
      <div className="rounded-md border border-border bg-background px-2 py-1">
        获取数据: <span className="text-foreground">{fetchTimeLabel}</span>
      </div>
    </div>
  </div>
  );
};

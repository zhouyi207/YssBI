import React from 'react';
import { useTranslation } from 'react-i18next';
import { VscChevronLeft, VscChevronRight, VscRefresh } from 'react-icons/vsc';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';

interface ToolbarProps {
  loading: boolean;
  totalRowCount: number;
  columnCount: number;
  pageIndex: number;
  pageSize: number;
  totalPages: number;
  lastFetchMs: number | null;
  /** 当前是否有打开的 DataFrame（用于导出等，与单元格是否选中无关） */
  exportEnabled: boolean;
  onPreviousPage: () => void;
  onNextPage: () => void;
  onRefresh: () => void;
  onExport: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  loading, totalRowCount, columnCount, pageIndex, pageSize, totalPages, lastFetchMs, exportEnabled,
  onPreviousPage, onNextPage, onRefresh, onExport,
}) => {
  const { t } = useTranslation();
  const pageStart = totalRowCount === 0 ? 0 : pageIndex * pageSize + 1;
  const pageEnd = Math.min(totalRowCount, (pageIndex + 1) * pageSize);
  const fetchTimeLabel = lastFetchMs === null ? '-' : lastFetchMs >= 1000 ? `${(lastFetchMs / 1000).toFixed(2)}s` : `${lastFetchMs}ms`;

  return (
  <div className="flex min-h-12 shrink-0 items-center gap-2 border-t border-border bg-card/90 px-3 py-2">
    <div className="flex shrink-0 items-center gap-1 rounded-md border border-border bg-background p-0.5 shadow-sm">
      <ToolbarIconButton type="button" variant="ghost" size="icon-sm" onClick={onRefresh} tooltip={t("common.refresh")}>
        <VscRefresh className={loading ? 'animate-spin' : ''} size={15} />
      </ToolbarIconButton>
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={onExport}
        disabled={!exportEnabled}
        tooltip={t("common.export")}
      >
        <svg className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
          <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
      </ToolbarIconButton>
    </div>

    <div className="flex min-w-0 flex-1 items-center justify-center">
    <div className="flex shrink-0 items-center overflow-hidden rounded-md border border-border bg-background text-card-foreground shadow-sm">
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        className="rounded-none border-r border-border"
        onClick={onPreviousPage}
        disabled={loading || pageIndex <= 0}
        tooltip="上一页"
      >
        <VscChevronLeft size={15} />
      </ToolbarIconButton>

      <div className="flex h-6 min-w-[184px] items-center justify-center gap-1 px-2 text-[11px]">
        <span className="font-medium text-foreground">{pageStart}-{pageEnd}</span>
        <span className="text-muted-foreground">/</span>
        <span className="text-muted-foreground">{totalRowCount}</span>
        <span className="ml-1 rounded-sm bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
          {pageIndex + 1} / {totalPages}
        </span>
      </div>

      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        className="rounded-none border-l border-border"
        onClick={onNextPage}
        disabled={loading || pageIndex >= totalPages - 1}
        tooltip="下一页"
      >
        <VscChevronRight size={15} />
      </ToolbarIconButton>
    </div>
    </div>

    <div className="hidden shrink-0 items-center gap-2 text-[10px] font-medium text-muted-foreground xl:flex">
      <div className="rounded-md border border-border bg-background px-2 py-1">
        {t("databaseEditor.columns").toUpperCase()}: <span className="text-foreground">{columnCount}</span>
      </div>
      <div className="rounded-md border border-border bg-background px-2 py-1">
        {t("databaseEditor.rows").toUpperCase()}: <span className="text-foreground">{totalRowCount}</span>
      </div>
      <div className="rounded-md border border-border bg-background px-2 py-1">
        获取数据: <span className="text-foreground">{fetchTimeLabel}</span>
      </div>
    </div>
  </div>
  );
};

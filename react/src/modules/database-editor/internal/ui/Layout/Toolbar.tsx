import React from "react";
import { useTranslation } from "react-i18next";
import { VscChevronLeft, VscChevronRight, VscExport, VscRefresh } from "react-icons/vsc";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";

interface ToolbarProps {
  loading: boolean;
  totalRowCount: number;
  columnCount: number;
  pageIndex: number;
  pageSize: number;
  totalPages: number;
  lastFetchMs: number | null;
  onPreviousPage: () => void;
  onNextPage: () => void;
  onRefresh: () => void;
  onExport: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  loading,
  totalRowCount,
  columnCount,
  pageIndex,
  pageSize,
  totalPages,
  lastFetchMs,
  onPreviousPage,
  onNextPage,
  onRefresh,
  onExport,
}) => {
  const { t } = useTranslation();
  const pageStart = totalRowCount === 0 ? 0 : pageIndex * pageSize + 1;
  const pageEnd = Math.min(totalRowCount, (pageIndex + 1) * pageSize);
  const fetchTimeLabel =
    lastFetchMs === null
      ? "-"
      : lastFetchMs >= 1000
        ? `${(lastFetchMs / 1000).toFixed(2)}s`
        : `${lastFetchMs}ms`;

  return (
    <div className="grid min-h-12 shrink-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border-t border-border bg-card/90 px-3 py-2 @[28rem]:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)]">
      <div className="flex items-center justify-self-start overflow-hidden rounded-md border border-border bg-background shadow-sm">
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          className="rounded-none border-r border-border"
          onClick={onRefresh}
          disabled={loading}
          aria-label={t("common.refresh")}
          tooltip={t("common.refresh")}
          side="top"
        >
          <VscRefresh className={loading ? "animate-spin" : ""} size={15} />
        </ToolbarIconButton>
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          className="rounded-none"
          onClick={onExport}
          aria-label={t("common.export")}
          tooltip={t("common.export")}
          side="top"
        >
          <VscExport size={15} />
        </ToolbarIconButton>
      </div>

      <div className="flex min-w-0 items-center justify-center">
        <div className="flex shrink-0 items-center overflow-hidden rounded-md border border-border bg-background text-card-foreground shadow-sm">
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-sm"
            className="rounded-none border-r border-border"
            onClick={onPreviousPage}
            disabled={loading || pageIndex <= 0}
            tooltip={t("databaseEditor.previousPage")}
          >
            <VscChevronLeft size={15} />
          </ToolbarIconButton>

          <div className="flex h-6 min-w-0 items-center justify-center gap-1 px-2 text-[11px] @[28rem]:min-w-[184px]">
            <span className="font-medium text-foreground">
              {pageStart}-{pageEnd}
            </span>
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
            tooltip={t("databaseEditor.nextPage")}
          >
            <VscChevronRight size={15} />
          </ToolbarIconButton>
        </div>
      </div>

      <div className="hidden min-w-0 items-center justify-self-end gap-2 whitespace-nowrap text-[10px] font-medium text-muted-foreground @[48rem]:flex">
        <div className="rounded-md border border-border bg-background px-2 py-1">
          {t("databaseEditor.columns").toUpperCase()}:{" "}
          <span className="text-foreground">{columnCount}</span>
        </div>
        <div className="rounded-md border border-border bg-background px-2 py-1">
          {t("databaseEditor.rows").toUpperCase()}:{" "}
          <span className="text-foreground">{totalRowCount}</span>
        </div>
        <div className="rounded-md border border-border bg-background px-2 py-1">
          {t("databaseEditor.fetchTime")}: <span className="text-foreground">{fetchTimeLabel}</span>
        </div>
      </div>
    </div>
  );
};

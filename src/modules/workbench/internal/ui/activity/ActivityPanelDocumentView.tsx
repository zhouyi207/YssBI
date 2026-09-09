import type { KeyboardEvent, MouseEvent, ReactNode } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscPackage, VscRefresh } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  ActivityActionId,
  ActivityItem,
  ActivityPanelDocument,
  ActivityPanelId,
  ActivityPanelRow,
  ActivityText,
  ActivityTool,
} from "@/shared/types/domain/activityPanel";
import { ActivityPanelShell } from "./ActivityPanelShell";
import { SidebarTreeCategoryRow } from "../sidebar/primitives/SidebarTreeCategoryRow";
import { SidebarSectionEmptyState } from "../sidebar/SidebarSectionEmptyState";
import { SidebarEmptyState } from "../sidebar/SidebarEmptyState";

const icons = { add: VscAdd, install: VscPackage, refresh: VscRefresh };
export function ActivityPanelDocumentView({
  panelId,
  document,
  error,
  expanded,
  onExpandedChange,
  busy = false,
  onRetry,
  actions = {},
  renderItem,
  onContextMenu,
  empty = false,
  notice,
}: {
  panelId: ActivityPanelId;
  document: ActivityPanelDocument | null;
  error?: string | null;
  expanded: Readonly<Record<string, boolean>>;
  onExpandedChange: (categoryId: string, expanded: boolean) => void;
  busy?: boolean;
  onRetry: () => void;
  actions?: Partial<Record<ActivityActionId, () => void>>;
  renderItem: (item: ActivityItem, depth: number) => ReactNode;
  onContextMenu?: (event: MouseEvent, row: ActivityPanelRow) => void;
  empty?: boolean;
  notice?: ReactNode;
}) {
  const { t } = useTranslation();
  const [focusedRowId, setFocusedRowId] = useState<string | null>(null);
  const label = (text: ActivityText) => ("key" in text ? t(text.key) : text.text);
  const toolbar = (tools: readonly ActivityTool[]) =>
    tools.map((tool) => {
      const Icon = icons[tool.icon];
      return (
        <Button
          key={tool.id}
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label(tool.label)}
          title={label(tool.label)}
          disabled={busy || !actions[tool.id]}
          onClick={(event) => {
            event.stopPropagation();
            actions[tool.id]?.();
          }}
        >
          <Icon aria-hidden />
        </Button>
      );
    });
  let collapsedDepth: number | null = null;
  const rows =
    document?.rows.filter((row) => {
      if (collapsedDepth !== null && row.depth > collapsedDepth) return false;
      collapsedDepth =
        row.kind === "category" && !(expanded[row.id] ?? row.defaultExpanded) ? row.depth : null;
      return true;
    }) ?? [];
  const tabStopId = rows.some((row) => row.id === focusedRowId) ? focusedRowId : rows[0]?.id;
  const navigate = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || event.altKey || event.ctrlKey || event.metaKey)
      return;
    const items = Array.from(
      event.currentTarget.parentElement!.querySelectorAll<HTMLElement>(
        ':scope > [role="treeitem"]',
      ),
    );
    const index = items.indexOf(event.currentTarget);
    const next = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: items.length - 1 }[
      event.key
    ];
    if (next === undefined) return;
    event.preventDefault();
    items[Math.max(0, Math.min(items.length - 1, next))]?.focus();
  };
  return (
    <ActivityPanelShell
      title={document ? label(document.title) : t(`activityBar.${panelId}`)}
      tools={document && toolbar(document.tools)}
    >
      {notice}
      {error ? (
        <div role="alert" className="px-3 py-2 text-xs text-destructive">
          {error}
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t("plugins.recheck")}
            onClick={onRetry}
          >
            <VscRefresh />
          </Button>
        </div>
      ) : null}
      {!document ? (
        !error && (
          <p role="status" className="px-3 py-3 text-xs text-muted-foreground">
            {t("common.loading")}
          </p>
        )
      ) : empty && document.emptyState ? (
        <SidebarEmptyState
          title={label(document.emptyState.title)}
          description={label(document.emptyState.description)}
        />
      ) : (
        <ScrollArea orientation="vertical" className="min-h-0 flex-1">
          <div role="tree" aria-label={label(document.title)}>
            {rows.map((row, index) => (
              <div
                key={row.id}
                role="treeitem"
                aria-level={row.depth + 1}
                aria-expanded={
                  row.kind === "category" ? (expanded[row.id] ?? row.defaultExpanded) : undefined
                }
                tabIndex={row.id === tabStopId ? 0 : -1}
                onFocus={() => setFocusedRowId(row.id)}
                onKeyDown={navigate}
                onContextMenu={(event) => {
                  if (row.kind === "message") {
                    for (let parent = index - 1; parent >= 0; parent--) {
                      if (rows[parent].kind === "category" && rows[parent].depth < row.depth) {
                        onContextMenu?.(event, rows[parent]);
                        return;
                      }
                    }
                  }
                  onContextMenu?.(event, row);
                }}
              >
                {row.kind === "category" ? (
                  <SidebarTreeCategoryRow
                    categoryId={row.id}
                    label={label(row.label)}
                    depth={row.depth}
                    expanded={expanded[row.id] ?? row.defaultExpanded}
                    onExpandedChange={(value) => onExpandedChange(row.id, value)}
                    trailing={
                      <>
                        {row.count !== null && (
                          <span className="px-1.5 text-[10px] text-muted-foreground">
                            {row.count}
                          </span>
                        )}
                        {toolbar(row.tools)}
                      </>
                    }
                  />
                ) : row.kind === "message" ? (
                  row.description ? (
                    <SidebarEmptyState
                      title={label(row.label)}
                      description={label(row.description)}
                    />
                  ) : (
                    <SidebarSectionEmptyState level={row.depth} message={label(row.label)} />
                  )
                ) : (
                  renderItem(row.item, row.depth)
                )}
              </div>
            ))}
          </div>
        </ScrollArea>
      )}
    </ActivityPanelShell>
  );
}

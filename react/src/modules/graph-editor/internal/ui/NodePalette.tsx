import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { Card } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { buildLocalizedCatalogBrowser } from "@/features/application/nodeCatalog/catalogTreeBrowser";
import { nodeCatalogErrorText } from "@/features/application/nodeCatalog/nodeCatalogErrorPresentation";
import { useCompatibleNodeCatalog } from "@/features/application/nodeCatalog/useCompatibleNodeCatalog";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import type { LocalizedCatalogBrowserRow } from "@/features/domain/nodeCatalog/localizedCatalogTree";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { useDismissableOverlay } from "@/shared/ui/positionedOverlay";
import { SidebarTreeSearchInput } from "@/modules/workbench/public";
import { NodeCreationForm } from "@/modules/details/public";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { formatInlineUserError } from "@/features/application/userErrorSummary";

export interface NodePaletteCatalogRowProps {
  readonly row: LocalizedCatalogBrowserRow;
  readonly expanded: boolean;
  readonly interactionDisabled?: boolean;
  readonly active?: boolean;
  readonly onExpandedChange: (expanded: boolean) => void;
  readonly onItemSelect?: (descriptor: NodeCreationDescriptor) => void;
}

export type NodePaletteCatalogRowRenderer = ComponentType<NodePaletteCatalogRowProps>;

export function NodePalette({
  x,
  y,
  graphPath = null,
  sourcePort = null,
  catalogRowRenderer: CatalogRowRenderer,
  onSelect,
  onClose,
}: {
  x: number;
  y: number;
  graphPath?: string | null;
  sourcePort?: PortAddressDto | null;
  catalogRowRenderer: NodePaletteCatalogRowRenderer;
  onSelect: (
    descriptor: NodeCreationDescriptor,
    locale: string,
    parameters?: Record<string, unknown>,
    portCounts?: Record<string, number>,
  ) => Promise<boolean> | void;
  onClose?: () => void;
}) {
  const { t } = useTranslation();
  const localized = useLocalizedNodeCatalog(sourcePort === null);
  const compatible = useCompatibleNodeCatalog({
    enabled: sourcePort !== null,
    graphPath,
    sourcePort,
  });
  const { status, error, catalog, searchIndex } = sourcePort ? compatible : localized;
  const [query, setQuery] = useState("");
  const [expandedCategoryIds, setExpandedCategoryIds] = useState<Set<string>>(new Set());
  const [activeItemKey, setActiveItemKey] = useState<string | null>(null);
  const paletteRef = useRef<HTMLDivElement>(null);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [configureFirst, setConfigureFirst] = useState(false);
  const [creationError, setCreationError] = useState<string | null>(null);
  const [configuration, setConfiguration] = useState<{
    descriptor: NodeCreationDescriptor;
    projectInstanceId: string;
    locale: string;
    title: string;
  } | null>(null);

  const select = useCallback(
    (descriptor: NodeCreationDescriptor) => {
      if (!catalog) return;
      setCreationError(null);
      if (configureFirst && projectInstanceId && graphPath) {
        const item = catalog.items.find((item) => item.creation === descriptor);
        setConfiguration({
          descriptor,
          projectInstanceId,
          locale: catalog.locale,
          title: item?.title ?? descriptor.nodeTypeId,
        });
        return;
      }
      void Promise.resolve(onSelect(descriptor, catalog.locale))
        .then((applied) => {
          if (applied === false) setCreationError(t("canvas.nodePalette.createFailed"));
        })
        .catch((error: unknown) => setCreationError(formatInlineUserError(error, t)));
    },
    [catalog, configureFirst, projectInstanceId, graphPath, onSelect, t],
  );

  useDismissableOverlay({ ref: paletteRef, onDismiss: onClose });

  useEffect(() => {
    if (configuration && configuration.projectInstanceId !== projectInstanceId)
      setConfiguration(null);
  }, [configuration, projectInstanceId]);

  useEffect(() => {
    setExpandedCategoryIds(
      catalog
        ? new Set(catalog.categories.map((category) => category.categoryId))
        : new Set<string>(),
    );
    setActiveItemKey(null);
  }, [catalog]);

  const projection = useMemo(
    () =>
      buildLocalizedCatalogBrowser({
        catalog,
        searchIndex,
        query,
        expandedCategoryIds,
      }),
    [catalog, expandedCategoryIds, query, searchIndex],
  );
  const itemRows = useMemo(
    () => projection.rows.filter((row) => row.kind === "item").filter((row) => row.item.available),
    [projection.rows],
  );
  const activeItem = itemRows.find((row) => row.rowKey === activeItemKey) ?? itemRows[0] ?? null;
  const setCategoryExpanded = useCallback(
    (categoryId: string, expanded: boolean) => {
      if (query.trim()) return;
      setActiveItemKey(null);
      setExpandedCategoryIds((current) => {
        const next = new Set(current);
        if (expanded) next.add(categoryId);
        else next.delete(categoryId);
        return next;
      });
    },
    [query],
  );
  const queryIsActive = query.trim().length > 0;
  const allCategoriesExpanded =
    projection.categoryIds.size > 0 &&
    [...projection.categoryIds].every((categoryId) => expandedCategoryIds.has(categoryId));
  const canToggleAllCategories = !queryIsActive && projection.categoryIds.size > 0;
  const toggleAllCategories = useCallback(() => {
    if (!canToggleAllCategories) return;
    setActiveItemKey(null);
    setExpandedCategoryIds((current) => {
      const next = new Set(current);
      for (const categoryId of projection.categoryIds) {
        if (allCategoriesExpanded) next.delete(categoryId);
        else next.add(categoryId);
      }
      return next;
    });
  }, [allCategoriesExpanded, canToggleAllCategories, projection.categoryIds]);

  const moveActiveItem = useCallback(
    (offset: number) => {
      if (!activeItem) return;
      const currentIndex = itemRows.findIndex((row) => row.rowKey === activeItem.rowKey);
      const nextIndex = Math.max(0, Math.min(itemRows.length - 1, currentIndex + offset));
      setActiveItemKey(itemRows[nextIndex]?.rowKey ?? null);
    },
    [activeItem, itemRows],
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (!(event.target instanceof HTMLInputElement) || event.nativeEvent.isComposing) return;
      if (event.key === "ArrowUp" || event.key === "ArrowDown") {
        event.preventDefault();
        event.stopPropagation();
        moveActiveItem(event.key === "ArrowUp" ? -1 : 1);
        return;
      }
      if (event.key === "Enter" && activeItem && catalog) {
        event.preventDefault();
        event.stopPropagation();
        select(activeItem.item.creation);
      }
    },
    [activeItem, catalog, moveActiveItem, select],
  );

  useEffect(() => {
    paletteRef.current
      ?.querySelector<HTMLElement>('[data-catalog-item-active="true"]')
      ?.scrollIntoView?.({ block: "nearest" });
  }, [activeItem?.rowKey]);

  return (
    <Card
      ref={paletteRef}
      className="menu-container fixed z-50 flex max-h-112 w-80 min-h-0 flex-col gap-1.5 overflow-hidden p-1.5 text-sm shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onKeyDown={configuration ? undefined : handleKeyDown}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {configuration && graphPath ? (
        <NodeCreationForm
          key={`${configuration.projectInstanceId}:${configuration.descriptor.nodeTypeId}`}
          projectInstanceId={configuration.projectInstanceId}
          nodeTypeId={configuration.descriptor.nodeTypeId}
          title={configuration.title}
          graphPath={graphPath}
          locale={configuration.locale}
          onBack={() => setConfiguration(null)}
          onCreate={async (parameters, portCounts) => {
            if (configuration.projectInstanceId !== useProjectIOStore.getState().projectInstanceId)
              return false;
            return (
              (await onSelect(
                configuration.descriptor,
                configuration.locale,
                parameters,
                portCounts,
              )) !== false
            );
          }}
        />
      ) : status === "error" && (!catalog || !searchIndex) ? (
        <p role="alert" className="px-2 py-1 text-destructive">
          {nodeCatalogErrorText(error, t)}
        </p>
      ) : !catalog || !searchIndex ? (
        <p role="status" className="px-2 py-1 text-muted-foreground">
          {t("common.loading")}
        </p>
      ) : (
        <>
          <div className="shrink-0 border-b border-border/60 px-0.5 pb-1.5">
            <SidebarTreeSearchInput
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                setActiveItemKey(null);
              }}
              autoFocus
              placeholder={t("canvas.nodePalette.searchPlaceholder")}
              expandAllLabel={t("canvas.nodePalette.expandAll")}
              collapseAllLabel={t("canvas.nodePalette.collapseAll")}
              allCategoriesExpanded={allCategoriesExpanded}
              canToggleAllCategories={canToggleAllCategories}
              onToggleAllCategories={toggleAllCategories}
            />
            {graphPath && (
              <label className="mt-2 flex items-center gap-2 text-xs">
                <input
                  type="checkbox"
                  checked={configureFirst}
                  onChange={(event) => setConfigureFirst(event.target.checked)}
                />
                {t("canvas.nodePalette.configureFirst")}
              </label>
            )}
          </div>
          {creationError && (
            <p role="alert" className="px-2 text-xs text-destructive">
              {creationError}
            </p>
          )}
          <span
            role="status"
            aria-atomic="true"
            data-node-palette-active-status
            className="sr-only"
          >
            {activeItem?.item.title ?? ""}
          </span>
          <ScrollArea className="max-h-80 min-h-0 flex-1">
            {projection.rows.length === 0 ? (
              <Empty className="gap-1 rounded-md px-2 py-4">
                <EmptyHeader>
                  <EmptyTitle className="text-xs font-normal text-muted-foreground">
                    {t("canvas.nodePalette.noMatches")}
                  </EmptyTitle>
                </EmptyHeader>
              </Empty>
            ) : (
              <div className="flex flex-col gap-0.5 pr-1">
                {projection.rows.map((row) => (
                  <CatalogRowRenderer
                    key={row.rowKey}
                    row={row}
                    expanded={
                      row.kind === "category" &&
                      projection.expandedCategoryIds.has(row.category.categoryId)
                    }
                    interactionDisabled={queryIsActive}
                    active={row.kind === "item" && row.rowKey === activeItem?.rowKey}
                    onExpandedChange={(expanded) => {
                      if (row.kind === "category") {
                        setCategoryExpanded(row.category.categoryId, expanded);
                      }
                    }}
                    onItemSelect={select}
                  />
                ))}
              </div>
            )}
          </ScrollArea>
        </>
      )}
    </Card>
  );
}

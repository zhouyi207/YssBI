import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  VscChevronRight,
  VscFolder,
  VscFolderOpened,
  VscSymbolMethod,
  VscSymbolProperty,
} from 'react-icons/vsc';
import { Collapsible, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Empty, EmptyHeader, EmptyTitle } from '@/components/ui/empty';
import { ScrollArea } from '@/components/ui/scroll-area';
import { buildLocalizedCatalogBrowser } from '@/features/application/nodeCatalog/catalogTreeBrowser';
import { nodeCatalogErrorText } from '@/features/application/nodeCatalog/nodeCatalogErrorPresentation';
import { useCompatibleNodeCatalog } from '@/features/application/nodeCatalog/useCompatibleNodeCatalog';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { LocalizedCatalogBrowserRow } from '@/features/domain/nodeCatalog/localizedCatalogTree';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { useDismissableOverlay } from '@/shared/ui/positionedOverlay';
import { cn } from '@/lib/utils';
import { SidebarTreeSearchInput } from './sidebarUi';

const PALETTE_ROW_INDENT = 14;
const PALETTE_ROW_LEADING_CLASS = 'flex size-3.5 shrink-0 items-center justify-center';

function paletteRowIndent(depth: number): React.CSSProperties {
  return { paddingLeft: 8 + depth * PALETTE_ROW_INDENT, paddingRight: 8 };
}

function PaletteTreeRow({
  row,
  expanded,
  interactionDisabled,
  locale,
  onExpandedChange,
  onSelect,
}: {
  row: LocalizedCatalogBrowserRow;
  expanded: boolean;
  interactionDisabled: boolean;
  locale: string;
  onExpandedChange: (expanded: boolean) => void;
  onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
}) {
  if (row.kind === 'category') {
    const FolderIcon = expanded ? VscFolderOpened : VscFolder;

    return (
      <Collapsible
        open={expanded}
        disabled={interactionDisabled}
        onOpenChange={onExpandedChange}
      >
        <CollapsibleTrigger asChild>
          <Button
            type="button"
            disabled={interactionDisabled}
            aria-disabled={interactionDisabled || undefined}
            variant="ghost"
            size="sm"
            data-catalog-category-id={row.category.categoryId}
            data-catalog-depth={row.depth}
            className={cn(
              'group h-7 w-full justify-start gap-1 rounded-md px-2 text-left text-xs font-medium transition-none',
              expanded
                ? 'bg-accent/70 text-accent-foreground'
                : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
            )}
            style={paletteRowIndent(row.depth)}
          >
            <VscChevronRight
              aria-hidden
              size={13}
              className="shrink-0 text-muted-foreground transition-transform duration-150 group-data-[state=open]:rotate-90"
            />
            <FolderIcon
              aria-hidden
              size={13}
              className={cn(
                'shrink-0 transition-colors',
                expanded ? 'text-primary' : 'text-muted-foreground',
              )}
            />
            <span className="min-w-0 flex-1 truncate">{row.category.title}</span>
          </Button>
        </CollapsibleTrigger>
      </Collapsible>
    );
  }

  const NodeIcon = row.item.creation.kind === 'resourceBound'
    ? VscSymbolMethod
    : VscSymbolProperty;

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      data-catalog-item-key={row.rowKey}
      className="h-8 w-full justify-start gap-2 rounded-md px-2 text-left font-normal text-foreground transition-none hover:bg-accent hover:text-accent-foreground"
      style={paletteRowIndent(row.depth)}
      title={row.item.nodeTypeId}
      onClick={() => onSelect(row.item.creation, locale)}
    >
      <span className={PALETTE_ROW_LEADING_CLASS} aria-hidden>
        <NodeIcon size={13} className="text-muted-foreground" />
      </span>
      <span className="min-w-0 flex-1 truncate">{row.item.title}</span>
    </Button>
  );
}

export function NodePalette({
  x,
  y,
  graphPath = null,
  graphRevision = null,
  sourcePort = null,
  onSelect,
  onClose,
}: {
  x: number;
  y: number;
  graphPath?: string | null;
  graphRevision?: number | null;
  sourcePort?: PortAddressDto | null;
  onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
  onClose?: () => void;
}) {
  const { t } = useTranslation();
  const localized = useLocalizedNodeCatalog(sourcePort === null);
  const compatible = useCompatibleNodeCatalog({
    enabled: sourcePort !== null,
    graphPath,
    graphRevision,
    sourcePort,
  });
  const { status, error, catalog, searchIndex } = sourcePort ? compatible : localized;
  const [query, setQuery] = useState('');
  const [expandedCategoryIds, setExpandedCategoryIds] = useState<Set<string>>(new Set());
  const paletteRef = useRef<HTMLDivElement>(null);

  useDismissableOverlay({ ref: paletteRef, onDismiss: onClose });

  useEffect(() => {
    setExpandedCategoryIds(catalog
      ? new Set(catalog.categories.map((category) => category.categoryId))
      : new Set<string>());
  }, [catalog]);

  const projection = useMemo(
    () => buildLocalizedCatalogBrowser({
      catalog,
      searchIndex,
      query,
      expandedCategoryIds,
    }),
    [catalog, expandedCategoryIds, query, searchIndex],
  );
  const setCategoryExpanded = useCallback((categoryId: string, expanded: boolean) => {
    if (query.trim()) return;
    setExpandedCategoryIds((current) => {
      const next = new Set(current);
      if (expanded) next.add(categoryId);
      else next.delete(categoryId);
      return next;
    });
  }, [query]);
  const queryIsActive = query.trim().length > 0;
  const allCategoriesExpanded = projection.categoryIds.size > 0
    && [...projection.categoryIds].every((categoryId) => expandedCategoryIds.has(categoryId));
  const canToggleAllCategories = !queryIsActive && projection.categoryIds.size > 0;
  const toggleAllCategories = useCallback(() => {
    if (!canToggleAllCategories) return;
    setExpandedCategoryIds((current) => {
      const next = new Set(current);
      for (const categoryId of projection.categoryIds) {
        if (allCategoriesExpanded) next.delete(categoryId);
        else next.add(categoryId);
      }
      return next;
    });
  }, [allCategoriesExpanded, canToggleAllCategories, projection.categoryIds]);

  return (
    <Card
      ref={paletteRef}
      className="menu-container fixed z-50 flex max-h-112 w-80 min-h-0 flex-col gap-1.5 overflow-hidden p-1.5 text-sm shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {status === 'error' && (!catalog || !searchIndex) ? (
        <p role="alert" className="px-2 py-1 text-destructive">
          {nodeCatalogErrorText(error, t)}
        </p>
      ) : !catalog || !searchIndex ? (
        <p role="status" className="px-2 py-1 text-muted-foreground">
          {t('common.loading')}
        </p>
      ) : (
        <>
          <div className="shrink-0 border-b border-border/60 px-0.5 pb-1.5">
            <SidebarTreeSearchInput
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              autoFocus
              placeholder={t('canvas.nodePalette.searchPlaceholder')}
              expandAllLabel={t('canvas.nodePalette.expandAll')}
              collapseAllLabel={t('canvas.nodePalette.collapseAll')}
              allCategoriesExpanded={allCategoriesExpanded}
              canToggleAllCategories={canToggleAllCategories}
              onToggleAllCategories={toggleAllCategories}
            />
          </div>
          <ScrollArea className="max-h-80 min-h-0 flex-1">
            {projection.rows.length === 0 ? (
              <Empty className="gap-1 rounded-md px-2 py-4">
                <EmptyHeader>
                  <EmptyTitle className="text-xs font-normal text-muted-foreground">
                    {t('canvas.nodePalette.noMatches')}
                  </EmptyTitle>
                </EmptyHeader>
              </Empty>
            ) : (
              <div className="flex flex-col gap-0.5 pr-1">
                {projection.rows.map((row) => (
                  <PaletteTreeRow
                    key={row.rowKey}
                    row={row}
                    locale={catalog.locale}
                    expanded={row.kind === 'category'
                      && projection.expandedCategoryIds.has(row.category.categoryId)}
                    interactionDisabled={queryIsActive}
                    onExpandedChange={(expanded) => {
                      if (row.kind === 'category') {
                        setCategoryExpanded(row.category.categoryId, expanded);
                      }
                    }}
                    onSelect={onSelect}
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

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  VscSearch,
  VscCircuitBoard,
  VscFold,
  VscExpandAll,
} from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import {
  NODE_CATALOG_ROW_HEIGHT,
  buildTreeFromItems,
  filterCatalogItems,
  flattenTree,
  type FlatRow,
  type NodeCatalogItem,
  type TreeCategory,
} from '@/features/domain/nodeCatalog';
import { SidebarChevron } from '../sidebarUi';
import { nodeCatalogItemIcon } from './nodeCatalogIcons';

export interface NodeCatalogTreeViewProps {
  items: NodeCatalogItem[];
  selectedKey?: string | null;
  onLeafClick?: (item: NodeCatalogItem) => void;
  className?: string;
  scrollClassName?: string;
  autoFocusSearch?: boolean;
  /** When false, parent supplies {@link filterQuery} and renders search externally. */
  showSearchBar?: boolean;
  /** Debounced filter string (used when showSearchBar is false). */
  filterQuery?: string;
}

function CatalogSearchBar({
  queryRaw,
  onQueryChange,
  inputRef,
  expandCollapseLabel,
  canExpandCollapse,
  isAnyCategoryExpanded,
  onToggleExpandAll,
}: {
  queryRaw: string;
  onQueryChange: (value: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  expandCollapseLabel: string;
  canExpandCollapse: boolean;
  isAnyCategoryExpanded: boolean;
  onToggleExpandAll: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="shrink-0 border-b border-border bg-muted/15 px-3 py-2.5">
      <div
        className={cn(
          'flex items-center gap-1 rounded-md border border-border/60 px-2 py-1 transition-[border-color,box-shadow]',
          'bg-background/90 shadow-xs focus-within:border-ring/50 focus-within:ring-2 focus-within:ring-ring/15',
        )}
      >
        <VscSearch className="ml-0.5 shrink-0 text-muted-foreground/70" size={13} aria-hidden />
        <Input
          ref={inputRef}
          value={queryRaw}
          onChange={(e) => onQueryChange(e.target.value)}
          placeholder={t('canvas.nodePalette.searchPlaceholder')}
          className="h-7 min-w-0 flex-1 border-0 bg-transparent px-1.5 text-[13px] shadow-none focus-visible:ring-0"
        />
        <span className="mx-0.5 h-4 w-px shrink-0 bg-border/60" aria-hidden />
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          disabled={!canExpandCollapse}
          onClick={onToggleExpandAll}
          aria-label={expandCollapseLabel}
          className={cn(
            'size-7 shrink-0 rounded-md text-muted-foreground transition-colors',
            'hover:bg-[var(--sidebar-hover)] hover:text-foreground',
            'disabled:opacity-35',
          )}
        >
          {isAnyCategoryExpanded ? <VscFold size={13} aria-hidden /> : <VscExpandAll size={13} aria-hidden />}
        </Button>
      </div>
    </div>
  );
}

export function NodeCatalogTreeView({
  items,
  selectedKey = null,
  onLeafClick,
  className,
  scrollClassName,
  autoFocusSearch = true,
  showSearchBar = true,
  filterQuery = '',
}: NodeCatalogTreeViewProps) {
  const { t } = useTranslation();
  const [queryRaw, setQueryRaw] = useState('');
  const internalQuery = useDebouncedValue(queryRaw, 150);
  const query = showSearchBar ? internalQuery : filterQuery;
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const root = useMemo(() => buildTreeFromItems(items), [items]);

  /** Stable key so expand defaults run on catalog change, not every parent re-render. */
  const catalogPathsKey = useMemo(
    () => [...root.allPaths].sort().join('\0'),
    [items],
  );

  const filteredTree = useMemo(() => {
    if (!query) return null;
    const matchingItems = filterCatalogItems(items, query);
    if (matchingItems.length === 0) {
      return { tree: null, allPaths: new Set<string>(), sortedChildren: [] as TreeCategory[] };
    }
    return buildTreeFromItems(matchingItems);
  }, [items, query]);

  const filteredPathsKey = useMemo(() => {
    if (!query || !filteredTree) return '';
    return [...filteredTree.allPaths].sort().join('\0');
  }, [query, filteredTree]);

  // Search: expand all matching category paths when the query changes.
  useEffect(() => {
    if (!query) return;
    setExpandedPaths(new Set(filteredPathsKey ? filteredPathsKey.split('\0') : []));
  }, [query, filteredPathsKey]);

  // Default: expand all categories when the palette opens or catalog changes (no active search).
  useEffect(() => {
    if (query) return;
    setExpandedPaths(new Set(catalogPathsKey ? catalogPathsKey.split('\0') : []));
  }, [catalogPathsKey, query]);

  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!showSearchBar || !autoFocusSearch) return;
    const timer = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(timer);
  }, [autoFocusSearch, showSearchBar]);

  const toggleExpand = useCallback((path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  }, []);

  const activeAllPaths = useMemo(
    () => (query && filteredTree ? filteredTree.allPaths : root.allPaths),
    [query, filteredTree, root.allPaths],
  );

  const isAnyCategoryExpanded = useMemo(
    () => [...activeAllPaths].some((path) => expandedPaths.has(path)),
    [activeAllPaths, expandedPaths],
  );

  const toggleExpandAll = useCallback(() => {
    setExpandedPaths((prev) => {
      const anyExpanded = [...activeAllPaths].some((path) => prev.has(path));
      return anyExpanded ? new Set<string>() : new Set(activeAllPaths);
    });
  }, [activeAllPaths]);

  const expandCollapseLabel = isAnyCategoryExpanded
    ? t('canvas.nodePalette.collapseAll')
    : t('canvas.nodePalette.expandAll');

  const activeChildren = query && filteredTree ? filteredTree.sortedChildren : root.sortedChildren;

  const flatRows = useMemo(() => {
    const rows: FlatRow[] = [];
    flattenTree(activeChildren, expandedPaths, '', 0, rows);
    return rows;
  }, [activeChildren, expandedPaths]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => NODE_CATALOG_ROW_HEIGHT,
    overscan: 15,
  });

  const noResults = query && (!filteredTree || !filteredTree.tree);

  const searchBar = (
    <CatalogSearchBar
      queryRaw={queryRaw}
      onQueryChange={setQueryRaw}
      inputRef={inputRef}
      expandCollapseLabel={expandCollapseLabel}
      canExpandCollapse={activeAllPaths.size > 0}
      isAnyCategoryExpanded={isAnyCategoryExpanded}
      onToggleExpandAll={toggleExpandAll}
    />
  );

  const treeContent = noResults ? (
    <div className="flex flex-1 items-center justify-center px-4 py-8 text-center text-[13px] text-muted-foreground/80">
      {t('canvas.nodePalette.noMatches')}
    </div>
  ) : (
    <OverlayScrollbar
      ref={scrollRef}
      direction="vertical"
      className={cn('min-h-0 min-w-0 flex-1 basis-0 py-1', scrollClassName)}
    >
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((vItem) => {
          const row = flatRows[vItem.index];
          return (
            <div
              key={vItem.key}
              className="absolute left-0 top-0 h-7 w-full"
              style={{ transform: `translateY(${vItem.start}px)` }}
            >
              {row.type === 'leaf' ? (
                <CatalogLeafRow
                  item={row.item}
                  level={row.level}
                  selected={selectedKey === row.item.nodeType}
                  onLeafClick={onLeafClick}
                />
              ) : (
                <CategoryRow
                  node={row.node}
                  path={row.path}
                  level={row.level}
                  expanded={expandedPaths.has(row.path)}
                  onToggle={toggleExpand}
                />
              )}
            </div>
          );
        })}
      </div>
    </OverlayScrollbar>
  );

  return (
    <div className={cn('flex min-h-0 flex-1 flex-col', className)}>
      {showSearchBar && searchBar}
      {treeContent}
    </div>
  );
}

const CatalogLeafRow = React.memo(function CatalogLeafRow({
  item,
  level,
  selected,
  onLeafClick,
}: {
  item: NodeCatalogItem;
  level: number;
  selected: boolean;
  onLeafClick?: (item: NodeCatalogItem) => void;
}) {
  if (!item?.nodeType) return null;

  const paddingLeft = (level + 1) * 12 + 12;

  return (
    <Button
      type="button"
      variant="ghost"
      className={cn(
        'flex h-full w-full cursor-pointer items-center justify-start gap-2 rounded-none px-3 py-1.5',
        selected && 'bg-[var(--sidebar-active)]',
      )}
      style={{ paddingLeft }}
      onClick={() => onLeafClick?.(item)}
    >
      {nodeCatalogItemIcon(item)}
      <span className="truncate text-xs">{item.title}</span>
    </Button>
  );
});

const CategoryRow = React.memo(function CategoryRow({
  node,
  path,
  level,
  expanded,
  onToggle,
}: {
  node: TreeCategory;
  path: string;
  level: number;
  expanded: boolean;
  onToggle: (path: string) => void;
}) {
  const paddingLeft = level * 12 + 8;

  return (
    <Button
      type="button"
      variant="ghost"
      className="flex h-full w-full cursor-pointer items-center justify-start gap-1 rounded-none px-2 py-1.5 font-bold text-foreground/80"
      style={{ paddingLeft }}
      onClick={() => onToggle(path)}
    >
      <SidebarChevron expanded={expanded} className="text-muted-foreground" />
      <VscCircuitBoard className="shrink-0 text-muted-foreground" size={14} />
      <span className="truncate text-[11px] font-bold uppercase tracking-wider text-foreground/80">
        {node.name}
      </span>
    </Button>
  );
});

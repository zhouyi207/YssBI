import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  VscSearch,
  VscSymbolMethod,
  VscSymbolVariable,
  VscCircuitBoard,
  VscSymbolProperty,
  VscFold,
  VscExpandAll,
} from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import {
  NODE_CATALOG_ROW_HEIGHT,
  buildTreeFromItems,
  flattenTree,
  type FlatRow,
  type NodeCatalogItem,
  type TreeCategory,
} from '@/features/domain/nodeCatalog';
import type { NodeTemplateDragData } from '@/features/core/dnd';
import {
  SidebarChevron,
  SidebarDraggableItem,
  nodeCatalogCategoryRowClass,
  nodeCatalogLeafLabelClass,
  nodeCatalogLeafRowClass,
  nodeCatalogSearchShellClass,
  sidebarItemIndent,
  sidebarSectionLabelClass,
} from '../sidebarUi';

function useDebouncedValue<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    timeoutRef.current = setTimeout(() => setDebouncedValue(value), delay);
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [value, delay]);

  return debouncedValue;
}

export interface NodeCatalogTreeViewProps {
  items: NodeCatalogItem[];
  variant?: 'sidebar' | 'popover';
  selectedKey?: string | null;
  onLeafClick?: (item: NodeCatalogItem) => void;
  getLeafDragData?: (item: NodeCatalogItem) => NodeTemplateDragData | null;
  className?: string;
  scrollClassName?: string;
  autoFocusSearch?: boolean;
}

function CatalogSearchBar({
  variant,
  queryRaw,
  onQueryChange,
  inputRef,
  expandCollapseLabel,
  canExpandCollapse,
  isAnyCategoryExpanded,
  onToggleExpandAll,
}: {
  variant: 'sidebar' | 'popover';
  queryRaw: string;
  onQueryChange: (value: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  expandCollapseLabel: string;
  canExpandCollapse: boolean;
  isAnyCategoryExpanded: boolean;
  onToggleExpandAll: () => void;
}) {
  const { t } = useTranslation();
  const isSidebar = variant === 'sidebar';

  return (
    <div
      className={cn(
        isSidebar ? nodeCatalogSearchShellClass() : 'shrink-0 border-b border-border bg-muted/15 px-3 py-2.5',
      )}
    >
      <div
        className={cn(
          'flex items-center gap-1 rounded-md border border-border/60 px-2 py-1 transition-[border-color,box-shadow]',
          isSidebar
            ? 'bg-sidebar/40 focus-within:border-sidebar-border focus-within:bg-sidebar/60'
            : 'bg-background/90 shadow-xs focus-within:border-ring/50 focus-within:ring-2 focus-within:ring-ring/15',
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
  variant = 'sidebar',
  selectedKey = null,
  onLeafClick,
  getLeafDragData,
  className,
  scrollClassName,
  autoFocusSearch = true,
}: NodeCatalogTreeViewProps) {
  const { t } = useTranslation();
  const [queryRaw, setQueryRaw] = useState('');
  const query = useDebouncedValue(queryRaw, 150);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const root = useMemo(() => buildTreeFromItems(items), [items]);

  const filteredTree = useMemo(() => {
    if (!query) return null;
    const q = query.toLowerCase();
    const matchingItems = items.filter(
      (item) =>
        item.title.toLowerCase().includes(q) ||
        item.nodeType.toLowerCase().includes(q) ||
        item.category.some((c) => c.toLowerCase().includes(q)),
    );
    if (matchingItems.length === 0) {
      return { tree: null, allPaths: new Set<string>(), sortedChildren: [] as TreeCategory[] };
    }
    return buildTreeFromItems(matchingItems);
  }, [items, query]);

  useEffect(() => {
    const paths = query && filteredTree ? filteredTree.allPaths : root.allPaths;
    if (paths.size === 0) return;
    setExpandedPaths((prev) => {
      if (prev.size === paths.size && [...paths].every((p) => prev.has(p))) return prev;
      return new Set(paths);
    });
  }, [query, filteredTree, root.allPaths]);

  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!autoFocusSearch) return;
    const timer = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(timer);
  }, [autoFocusSearch]);

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
  const isSidebar = variant === 'sidebar';

  const searchBar = (
    <CatalogSearchBar
      variant={variant}
      queryRaw={queryRaw}
      onQueryChange={setQueryRaw}
      inputRef={inputRef}
      expandCollapseLabel={expandCollapseLabel}
      canExpandCollapse={activeAllPaths.size > 0}
      isAnyCategoryExpanded={isAnyCategoryExpanded}
      onToggleExpandAll={toggleExpandAll}
    />
  );

  const treeContent =
    noResults ? (
      <div className="flex flex-1 items-center justify-center px-4 py-8 text-center text-[13px] text-muted-foreground/80">
        {t('canvas.nodePalette.noMatches')}
      </div>
    ) : (
      <OverlayScrollbar
        ref={scrollRef}
        direction="vertical"
        className={cn('min-h-0 flex-1', isSidebar ? 'py-0.5' : 'max-h-96 py-1', scrollClassName)}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((vItem) => {
            const row = flatRows[vItem.index];
            return (
              <div
                key={vItem.key}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: `${vItem.size}px`,
                  transform: `translateY(${vItem.start}px)`,
                }}
              >
                {row.type === 'leaf' ? (
                  <CatalogLeafRow
                    item={row.item}
                    level={row.level}
                    variant={variant}
                    selected={selectedKey === row.item.nodeType}
                    onLeafClick={onLeafClick}
                    dragData={getLeafDragData?.(row.item) ?? null}
                  />
                ) : (
                  <CategoryRow
                    node={row.node}
                    path={row.path}
                    level={row.level}
                    expanded={expandedPaths.has(row.path)}
                    onToggle={toggleExpand}
                    variant={variant}
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
      {!isSidebar && searchBar}
      {treeContent}
      {isSidebar && searchBar}
    </div>
  );
}

const CatalogLeafRow = React.memo(function CatalogLeafRow({
  item,
  level,
  variant,
  selected,
  onLeafClick,
  dragData,
}: {
  item: NodeCatalogItem;
  level: number;
  variant: 'sidebar' | 'popover';
  selected: boolean;
  onLeafClick?: (item: NodeCatalogItem) => void;
  dragData: NodeTemplateDragData | null;
}) {
  if (!item?.nodeType) return null;

  const icon = item.nodeType.includes('Variable') ? (
    <VscSymbolVariable className="text-blue-400/90" size={14} />
  ) : item.nodeType.includes('Call') ? (
    <VscSymbolMethod className="text-purple-400/90" size={14} />
  ) : (
    <VscSymbolProperty className="text-[var(--accent-color)]" size={14} />
  );

  if (variant === 'sidebar') {
    return (
      <SidebarDraggableItem
        id={`catalog-${item.nodeType}`}
        dragData={dragData}
        className={cn(nodeCatalogLeafRowClass(selected), 'cursor-grab active:cursor-grabbing')}
        style={sidebarItemIndent(level)}
        onClick={() => onLeafClick?.(item)}
      >
        <span className="flex size-4 shrink-0 items-center justify-center">{icon}</span>
        <span className={nodeCatalogLeafLabelClass(selected)}>{item.title}</span>
      </SidebarDraggableItem>
    );
  }

  const paddingLeft = (level + 1) * 12 + 12;
  return (
    <Button
      type="button"
      variant="ghost"
      className="flex h-full w-full cursor-pointer items-center justify-start gap-2 rounded-none px-3 py-1.5"
      style={{ paddingLeft }}
      onClick={() => onLeafClick?.(item)}
    >
      {icon}
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
  variant,
}: {
  node: TreeCategory;
  path: string;
  level: number;
  expanded: boolean;
  onToggle: (path: string) => void;
  variant: 'sidebar' | 'popover';
}) {
  if (variant === 'sidebar') {
    return (
      <div
        role="button"
        tabIndex={0}
        onClick={() => onToggle(path)}
        onKeyDown={(e) => {
          if (e.key !== 'Enter' && e.key !== ' ') return;
          e.preventDefault();
          onToggle(path);
        }}
        className={nodeCatalogCategoryRowClass()}
        style={sidebarItemIndent(level)}
      >
        <SidebarChevron expanded={expanded} />
        <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">
          <VscCircuitBoard size={14} />
        </span>
        <span className={sidebarSectionLabelClass()}>{node.name}</span>
        <span className="size-6 shrink-0" aria-hidden />
      </div>
    );
  }

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

import React, { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useNodeRegistryStore } from "@/features/core/nodeRegister";
import { Pin, Node, Variable, Graph } from "@/shared/types/domain";
import { isNodeCompatibleWithPin, pinAcceptsType, buildPinDataType } from "@/shared/utils/pinCompatibility";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { VscChevronRight, VscChevronDown, VscSearch, VscSymbolMethod, VscSymbolVariable, VscCircuitBoard, VscSymbolProperty, VscFold, VscExpandAll } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/** PaletteItem overrides 扩展类型 */
export interface PaletteItemOverrides extends Partial<Node> {
  subGraphId?: string;
  variableId?: string;
}

export interface PaletteItem {
  nodeType: string;
  title: string;
  category: string[];
  overrides?: PaletteItemOverrides;
}

interface TreeCategory {
  name: string;
  isLeaf: false;
  children: Record<string, TreeNode>;
}

interface TreeLeaf {
  name: string;
  isLeaf: true;
  item: PaletteItem;
}

type TreeNode = TreeCategory | TreeLeaf;

type FlatRow =
  | { type: 'category'; level: number; node: TreeCategory; path: string }
  | { type: 'leaf'; level: number; item: PaletteItem };

const ROW_HEIGHT = 30;

const TREE_SORT = (a: TreeNode, b: TreeNode) => {
  if (a.isLeaf !== b.isLeaf) return a.isLeaf ? 1 : -1;
  return a.name.localeCompare(b.name);
};

function buildTreeFromItems(items: PaletteItem[]): { tree: TreeCategory; allPaths: Set<string>; sortedChildren: TreeNode[] } {
  const tree: TreeCategory = { name: "Root", isLeaf: false, children: {} };
  const allPaths = new Set<string>();

  items.forEach((item) => {
    let current = tree;
    let path = "";
    item.category.forEach((cat) => {
      path = path ? `${path}/${cat}` : cat;
      allPaths.add(path);
      if (!current.children[cat]) {
        current.children[cat] = { name: cat, isLeaf: false, children: {} };
      }
      current = current.children[cat] as TreeCategory;
    });
    const leafKey = `${item.nodeType}-${item.overrides?.variableId ?? item.overrides?.subGraphId ?? ""}`;
    current.children[leafKey] = { name: item.title, isLeaf: true, item };
  });

  const sortedChildren = Object.values(tree.children).sort(TREE_SORT);
  return { tree, allPaths, sortedChildren };
}

function flattenTree(
  children: TreeNode[],
  expandedPaths: Set<string>,
  parentPath: string,
  level: number,
  out: FlatRow[],
) {
  const sorted = [...children].sort(TREE_SORT);
  for (const child of sorted) {
    if (child.isLeaf) {
      out.push({ type: 'leaf', level, item: child.item });
    } else {
      const path = parentPath ? `${parentPath}/${child.name}` : child.name;
      out.push({ type: 'category', level, node: child, path });
      if (expandedPaths.has(path)) {
        flattenTree(Object.values(child.children), expandedPaths, path, level + 1, out);
      }
    }
  }
}

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

export function NodePalette({
  x,
  y,
  onSelect,
  filterPin,
  variables = {},
  functions = {},
}: {
  x: number;
  y: number;
  onSelect: (item: PaletteItem) => void;
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, Graph>;
}) {
  const { t } = useTranslation();
  const [queryRaw, setQueryRaw] = useState("");
  const query = useDebouncedValue(queryRaw, 150);
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const variableKeysStr = useMemo(
    () => Object.keys(variables).sort().join(","),
    [variables]
  );
  const functionKeysStr = useMemo(
    () => Object.keys(functions).sort().join(","),
    [functions]
  );
  const allItems = useMemo(() => {
    const items: PaletteItem[] = [];

    definitions.forEach((node) => {
      if (["Variables:Get Variable", "Variables:Set Variable", "Functions:Call Function"].includes(node.nodeType)) return;

      if (filterPin) {
        if (!isNodeCompatibleWithPin(node, filterPin)) return;
      }

      items.push({ nodeType: node.nodeType, title: node.name, category: node.category || [] });
    });

    Object.values(variables).forEach((v) => {
      if (!v?.name || !v?.id) return;
      const varName = v.name;
      const varId = v.id;
      const varType = v.dataType;

      let getCompatible = true;
      if (filterPin) {
        if (filterPin.direction === "output") getCompatible = false;
        else getCompatible = pinAcceptsType(filterPin, varType);
      }
      if (getCompatible) {
        items.push({
          nodeType: "Variables:Get Variable",
          title: `Get ${varName}`,
          category: ["Variables"],
          overrides: { title: "Get Variable", variableId: varId },
        });
      }

      let setCompatible = true;
      if (filterPin) {
        if (filterPin.direction === "input") setCompatible = false;
        else setCompatible = pinAcceptsType(filterPin, varType);
      }
      if (setCompatible) {
        items.push({
          nodeType: "Variables:Set Variable",
          title: `Set ${varName}`,
          category: ["Variables"],
          overrides: { title: "Set Variable", variableId: varId },
        });
      }
    });

    Object.values(functions).forEach((sub) => {
      if (!sub?.name || !sub?.id) return;
      if (filterPin && filterPin.type !== "exec") {
        const targetPins = filterPin.direction === "input" ? sub.outputs : sub.inputs;
        const hasCompatible = (targetPins || []).some(
          (p: Pin) => p.type !== "exec" && pinAcceptsType(filterPin, buildPinDataType(p))
        );
        if (!hasCompatible) return;
      }
      items.push({
        nodeType: "Functions:Call Function",
        title: `Call ${sub.name}`,
        category: ["Functions"],
        overrides: { subGraphId: sub.id, title: sub.name },
      });
    });

    return items;
  }, [filterPin, variableKeysStr, functionKeysStr, definitions]);

  const root = useMemo(() => buildTreeFromItems(allItems), [allItems]);

  const filteredTree = useMemo(() => {
    if (!query) return null;
    const q = query.toLowerCase();
    const matchingItems = allItems.filter(
      (item) =>
        item.title.toLowerCase().includes(q) ||
        item.category.some((c) => c.toLowerCase().includes(q))
    );
    if (matchingItems.length === 0) return { tree: null, allPaths: new Set<string>(), sortedChildren: [] as TreeNode[] };
    return buildTreeFromItems(matchingItems);
  }, [allItems, query]);

  useEffect(() => {
    const paths = query && filteredTree ? filteredTree.allPaths : root.allPaths;
    if (paths.size === 0) return;
    setExpandedPaths((prev) => {
      if (prev.size === paths.size && [...paths].every((p) => prev.has(p))) return prev;
      return new Set(paths);
    });
  }, [query]);

  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const t = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(t);
  }, []);

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
    ? t("canvas.nodePalette.collapseAll")
    : t("canvas.nodePalette.expandAll");

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
    estimateSize: () => ROW_HEIGHT,
    overscan: 15,
  });

  const noResults = query && (!filteredTree || !filteredTree.tree);

  return (
    <Card
      className="menu-container fixed z-50 flex w-80 flex-col overflow-hidden shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <div className="border-b border-border bg-muted/15 px-3 py-2.5">
        <div className="flex items-center gap-1 rounded-lg border border-border/70 bg-background/90 px-2 py-1 shadow-xs transition-[border-color,box-shadow] focus-within:border-ring/50 focus-within:ring-2 focus-within:ring-ring/15">
          <VscSearch className="ml-0.5 shrink-0 text-muted-foreground/75" size={13} aria-hidden />
          <Input
            ref={inputRef}
            value={queryRaw}
            onChange={(e) => setQueryRaw(e.target.value)}
            placeholder={t("canvas.nodePalette.searchPlaceholder")}
            className="h-6 min-w-0 flex-1 border-0 bg-transparent px-1.5 text-xs shadow-none focus-visible:ring-0"
          />
          <span className="mx-0.5 h-4 w-px shrink-0 bg-border/80" aria-hidden />
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            disabled={activeAllPaths.size === 0}
            onClick={toggleExpandAll}
            aria-label={expandCollapseLabel}
            className={cn(
              "size-6 shrink-0 rounded-md text-muted-foreground transition-colors",
              "hover:bg-muted/80 hover:text-foreground",
              "disabled:opacity-35",
            )}
          >
            {isAnyCategoryExpanded ? (
              <VscFold size={13} aria-hidden />
            ) : (
              <VscExpandAll size={13} aria-hidden />
            )}
          </Button>
        </div>
      </div>

      {noResults ? (
        <div className="px-4 py-8 text-center text-xs italic text-muted-foreground">
          {t("canvas.nodePalette.noMatches")}
        </div>
      ) : (
        <OverlayScrollbar ref={scrollRef} direction="vertical" className="max-h-96 py-1">
          <div
            style={{ height: virtualizer.getTotalSize(), position: 'relative' }}
          >
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
                    <LeafRow item={row.item} level={row.level} onSelect={onSelect} />
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
      )}
    </Card>
  );
}

const LeafRow = React.memo(function LeafRow({
  item,
  level,
  onSelect,
}: {
  item: PaletteItem;
  level: number;
  onSelect: (item: PaletteItem) => void;
}) {
  if (!item?.nodeType) return null;
  const paddingLeft = (level + 1) * 12 + 12;
  return (
    <Button
      type="button"
      variant="ghost"
      className="flex h-full w-full cursor-pointer items-center justify-start gap-2 rounded-none px-3 py-1.5"
      style={{ paddingLeft }}
      onClick={() => onSelect(item)}
    >
      {item.nodeType.includes("Variable") ? (
        <VscSymbolVariable className="text-blue-400 shrink-0" size={14} />
      ) : item.nodeType.includes("Call") ? (
        <VscSymbolMethod className="text-purple-400 shrink-0" size={14} />
      ) : (
        <VscSymbolProperty className="text-[var(--accent-color)] shrink-0" size={14} />
      )}
      <span className="text-xs truncate">{item.title}</span>
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
  return (
    <Button
      type="button"
      variant="ghost"
      className="flex h-full w-full cursor-pointer items-center justify-start gap-1 rounded-none px-2 py-1.5 font-bold text-foreground/80"
      style={{ paddingLeft: level * 12 + 8 }}
      onClick={() => onToggle(path)}
    >
      {expanded ? (
        <VscChevronDown size={14} className="text-muted-foreground" />
      ) : (
        <VscChevronRight size={14} className="text-muted-foreground" />
      )}
      <VscCircuitBoard className="shrink-0 text-muted-foreground" size={14} />
      <span className="text-[11px] uppercase tracking-wider truncate">{node.name}</span>
    </Button>
  );
});

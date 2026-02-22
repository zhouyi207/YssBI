import React, { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useNodeRegistryStore } from "@/features/core/nodeRegister";
import { Pin, Node, Variable, Graph } from "@/shared/types/domain";
import { dataTypeMatches, dataTypeDisplay } from "@/shared/types/domain/dataType";
import { isNodeCompatibleWithPin } from "@/shared/utils/pinCompatibility";
import { VscChevronRight, VscChevronDown, VscSearch, VscSymbolMethod, VscSymbolVariable, VscCircuitBoard, VscSymbolProperty } from "react-icons/vsc";

/** PaletteItem overrides 扩展类型 */
export interface PaletteItemOverrides extends Partial<Node> {
  subGraphId?: string;
  variableId?: string;
  variableName?: string;
  variableType?: string;
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
  Variables = {},
  functions = {},
  macros = {},
}: {
  x: number;
  y: number;
  onSelect: (item: PaletteItem) => void;
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  Variables?: Record<string, Variable>;
  functions?: Record<string, Graph>;
  macros?: Record<string, Graph>;
}) {
  const [queryRaw, setQueryRaw] = useState("");
  const query = useDebouncedValue(queryRaw, 150);
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const variableKeysStr = useMemo(
    () => Object.keys(variables).sort().join(","),
    [variables]
  );
  const globalVariableKeysStr = useMemo(
    () => Object.keys(Variables).sort().join(","),
    [Variables]
  );
  const functionKeysStr = useMemo(
    () => Object.keys(functions).sort().join(","),
    [functions]
  );
  const macroKeysStr = useMemo(
    () => Object.keys(macros).sort().join(","),
    [macros]
  );

  const allItems = useMemo(() => {
    const items: PaletteItem[] = [];

    definitions.forEach((node) => {
      if (["get_variable", "set_variable", "call_function", "call_macro"].includes(node.name)) return;

      if (filterPin) {
        if (!isNodeCompatibleWithPin(node, filterPin)) return;
      }

      items.push({ nodeType: node.nodeType, title: node.name, category: node.category || [] });
    });

    const allVars = { ...Variables, ...variables };
    Object.values(allVars).forEach((v) => {
      if (!v?.name || !v?.id) return;
      const varName = v.name;
      const varId = v.id;
      const varType = v.dataType;

      let getCompatible = true;
      if (filterPin) {
        if (filterPin.direction === "output") getCompatible = false;
        else getCompatible = dataTypeMatches(varType, filterPin.type) || filterPin.type === "any";
      }
      if (getCompatible) {
        items.push({
          nodeType: "get_variable",
          title: `Get ${varName}`,
          category: ["Variables"],
          overrides: { title: `Get ${varName}`, variableId: varId, variableName: varName, variableType: dataTypeDisplay(varType) },
        });
      }

      let setCompatible = true;
      if (filterPin) {
        if (filterPin.direction === "input") setCompatible = false;
        else setCompatible = dataTypeMatches(varType, filterPin.type) || filterPin.type === "any";
      }
      if (setCompatible) {
        items.push({
          nodeType: "set_variable",
          title: `Set ${varName}`,
          category: ["Variables"],
          overrides: { title: `Set ${varName}`, variableId: varId, variableName: varName, variableType: dataTypeDisplay(varType) },
        });
      }
    });

    const processGraphs = (collection: Record<string, any>, type: "function" | "macro") => {
      Object.values(collection).forEach((sub) => {
        if (!sub?.name || !sub?.id) return;
        if (filterPin) {
          const targetPins = filterPin.direction === "input" ? sub.outputs : sub.inputs;
          const hasCompatible = (targetPins || []).some(
            (p: any) => p.type === filterPin.type || p.type === "any" || filterPin.type === "any"
          );
          if (!hasCompatible && filterPin.type !== "exec") return;
        }
        items.push({
          nodeType: type === "function" ? "call_function" : "call_macro",
          title: `${type === "function" ? "Call" : "Macro"} ${sub.name}`,
          category: type === "function" ? ["Functions"] : ["Macros"],
          overrides: { subGraphId: sub.id, title: sub.name },
        });
      });
    };
    processGraphs(functions, "function");
    processGraphs(macros, "macro");

    return items;
  }, [filterPin, variableKeysStr, globalVariableKeysStr, functionKeysStr, macroKeysStr, definitions]);

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
    <div
      className="fixed z-50 w-80 bg-[#1e1e1e] text-[#cccccc] rounded shadow-2xl overflow-hidden border border-[#333333] flex flex-col menu-container animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <div className="flex items-center px-3 py-2 bg-[#252526] border-b border-[#333333]">
        <VscSearch className="text-gray-500 mr-2" size={14} />
        <input
          ref={inputRef}
          value={queryRaw}
          onChange={(e) => setQueryRaw(e.target.value)}
          placeholder="Search nodes..."
          className="w-full bg-transparent outline-none text-xs"
        />
      </div>

      {noResults ? (
        <div className="px-4 py-8 text-center text-xs text-gray-600 italic">No matches found</div>
      ) : (
        <div
          ref={scrollRef}
          className="max-h-96 overflow-y-auto py-1"
          style={{ scrollbarWidth: 'thin', scrollbarColor: '#555 transparent' }}
        >
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
        </div>
      )}
    </div>
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
    <div
      className="px-3 py-1.5 hover:bg-[#2a2d2e] cursor-pointer flex items-center gap-2 transition-colors h-full"
      style={{ paddingLeft }}
      onClick={() => onSelect(item)}
    >
      {item.nodeType.includes("variable") ? (
        <VscSymbolVariable className="text-blue-400 shrink-0" size={14} />
      ) : item.nodeType.includes("call") ? (
        <VscSymbolMethod className="text-purple-400 shrink-0" size={14} />
      ) : (
        <VscSymbolProperty className="text-[var(--accent-color)] shrink-0" size={14} />
      )}
      <span className="text-xs truncate">{item.title}</span>
    </div>
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
    <div
      className="px-2 py-1.5 hover:bg-[#252526] cursor-pointer flex items-center gap-1 transition-colors text-gray-300 font-bold h-full"
      style={{ paddingLeft: level * 12 + 8 }}
      onClick={() => onToggle(path)}
    >
      {expanded ? (
        <VscChevronDown size={14} className="text-gray-500" />
      ) : (
        <VscChevronRight size={14} className="text-gray-500" />
      )}
      <VscCircuitBoard className="text-gray-500 shrink-0" size={14} />
      <span className="text-[11px] uppercase tracking-wider truncate">{node.name}</span>
    </div>
  );
});

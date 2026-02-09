import React, { useState, useMemo, useEffect } from "react";
import { useNodeDefinitions } from "@/features/node-registry";
import { Pin, BaseNode } from "../Types/nodes";
import { VariableDefinition } from "../Types/variables";
import { VscChevronRight, VscChevronDown, VscSearch, VscSymbolMethod, VscSymbolVariable, VscCircuitBoard, VscSymbolProperty } from "react-icons/vsc";

export interface PaletteItem {
  type: string;
  title: string;
  category: string[];
  overrides?: Partial<BaseNode>;
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

export default function NodePalette({
  x,
  y,
  onSelect,
  filterPin,
  variables = {},
  globalVariables = {},
  functions = {},
  macros = {},
}: {
  x: number;
  y: number;
  onSelect: (item: PaletteItem) => void;
  filterPin?: Pin | null;
  variables?: Record<string, VariableDefinition>;
  globalVariables?: Record<string, VariableDefinition>;
  functions?: Record<string, import("../Types/canvas").SubGraphData>;
  macros?: Record<string, import("../Types/canvas").SubGraphData>;
}) {
  const [query, setQuery] = useState("");
  const { definitions } = useNodeDefinitions();
  // 记录哪些文件夹是展开的
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  // 1. 获取所有项
  const allItems = useMemo(() => {
    const items: PaletteItem[] = [];

    // 处理注册表中的常规节点 (数学, 逻辑, 调试等)
    definitions.forEach((node) => {
      // 排除需要在下方特殊处理的模板节点
      if (['get_variable', 'set_variable', 'call_function', 'call_macro'].includes(node.node_type)) return;

      if (filterPin) {
        const targetDirection = filterPin.direction === "input" ? "outputs" : "inputs";
        const pins = node[targetDirection] || [];
        const hasCompatiblePin = pins.some(p => p.type === filterPin.type || p.type === 'any' || filterPin.type === 'any');
        if (!hasCompatiblePin) return;
      }

      items.push({
        type: node.node_type,
        title: node.title,
        category: node.category,
      });
    });

    // 处理变量 (Get/Set)
    const allVars = { ...globalVariables, ...variables };
    Object.values(allVars).forEach((v) => {
      const varName = v.name;
      const varId = v.id;
      const varType = v.data_type;

      // Get 节点
      let getCompatible = true;
      if (filterPin) {
        if (filterPin.direction === 'output') getCompatible = false;
        else getCompatible = (varType === filterPin.type || filterPin.type === 'any' || filterPin.type === 'any');
      }
      if (getCompatible) {
        items.push({
          type: 'get_variable',
          title: `Get ${varName}`,
          category: ['Variables'],
          overrides: { title: `Get ${varName}`, variableId: varId, variableName: varName, variableType: varType } as any
        });
      }

      // Set 节点
      let setCompatible = true;
      if (filterPin) {
        if (filterPin.direction === 'input') setCompatible = false;
        else setCompatible = (varType === filterPin.type || filterPin.type === 'any' || filterPin.type === 'any');
      }
      if (setCompatible) {
        items.push({
          type: 'set_variable',
          title: `Set ${varName}`,
          category: ['Variables'],
          overrides: { title: `Set ${varName}`, variableId: varId, variableName: varName, variableType: varType } as any
        });
      }
    });

    // 处理函数和宏 (Call)
    const processSubGraphs = (collection: Record<string, any>, type: 'function' | 'macro') => {
      Object.values(collection).forEach(sub => {
        if (filterPin) {
          const targetPins = filterPin.direction === 'input' ? sub.outputs : sub.inputs;
          const hasCompatible = (targetPins || []).some((p: any) => p.type === filterPin.type || p.type === 'any' || filterPin.type === 'any');
          const hasExec = filterPin.type === 'exec';
          if (!hasCompatible && !hasExec) return;
        }

        items.push({
          type: type === 'function' ? 'call_function' : 'call_macro',
          title: `${type === 'function' ? 'Call' : 'Macro'} ${sub.name}`,
          category: type === 'function' ? ['Functions'] : ['Macros'],
          overrides: { subGraphId: sub.id, title: sub.name } as any
        });
      });
    };
    processSubGraphs(functions, 'function');
    processSubGraphs(macros, 'macro');

    return items;
  }, [filterPin, variables, globalVariables, functions, macros]);

  // 2. 构建树结构
  const root = useMemo(() => {
    const tree: TreeCategory = { name: "Root", isLeaf: false, children: {} };
    const allPaths = new Set<string>();

    allItems.forEach(item => {
      let current = tree;
      let path = "";
      
      item.category.forEach(cat => {
        path = path ? `${path}/${cat}` : cat;
        allPaths.add(path);
        if (!current.children[cat]) {
          current.children[cat] = { name: cat, isLeaf: false, children: {} };
        }
        current = current.children[cat] as TreeCategory;
      });

      const leafName = item.title;
      current.children[leafName] = { name: leafName, isLeaf: true, item };
    });

    return { tree, allPaths };
  }, [allItems]);

  // 初始时展开所有路径
  useEffect(() => {
    if (root.allPaths.size > 0) {
      setExpandedPaths(new Set(root.allPaths));
    }
  }, [root.allPaths]);

  // 聚焦搜索框
  const inputRef = React.useRef<HTMLInputElement>(null);
  useEffect(() => {
    setTimeout(() => inputRef.current?.focus(), 50);
  }, []);

  // 搜索结果
  const searchResults = useMemo(() => {
    if (!query) return null;
    return allItems.filter(item => 
      item.title.toLowerCase().includes(query.toLowerCase()) || 
      item.category.some(c => c.toLowerCase().includes(query.toLowerCase()))
    );
  }, [allItems, query]);

  const toggleExpand = (path: string) => {
    const next = new Set(expandedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setExpandedPaths(next);
  };

  // 递归渲染树
  const renderTreeNode = (node: TreeNode, path: string, level: number) => {
    if (node.isLeaf) {
      return (
        <div
          key={`${path}/${node.name}`}
          className="px-3 py-1.5 hover:bg-[#2a2d2e] cursor-pointer group flex items-center gap-2 transition-colors"
          style={{ paddingLeft: `${(level + 1) * 12 + 12}px` }}
          onClick={() => onSelect(node.item)}
        >
          {node.item.type.includes('variable') ? (
            <VscSymbolVariable className="text-blue-400 shrink-0" size={14} />
          ) : node.item.type.includes('call') ? (
            <VscSymbolMethod className="text-purple-400 shrink-0" size={14} />
          ) : (
            <VscSymbolProperty className="text-[var(--accent-color)] shrink-0" size={14} />
          )}
          <span className="text-xs truncate">{node.name}</span>
        </div>
      );
    }

    const currentPath = path ? `${path}/${node.name}` : node.name;
    const isExpanded = expandedPaths.has(currentPath);

    return (
      <div key={currentPath}>
        <div
          className="px-2 py-1.5 hover:bg-[#252526] cursor-pointer flex items-center gap-1 transition-colors text-gray-300 font-bold"
          style={{ paddingLeft: `${level * 12 + 8}px` }}
          onClick={() => toggleExpand(currentPath)}
        >
          {isExpanded ? <VscChevronDown size={14} className="text-gray-500" /> : <VscChevronRight size={14} className="text-gray-500" />}
          <VscCircuitBoard className="text-gray-500 shrink-0" size={14} />
          <span className="text-[11px] uppercase tracking-wider truncate">{node.name}</span>
        </div>
        {isExpanded && (
          <div>
            {Object.values(node.children)
              .sort((a, b) => {
                if (a.isLeaf !== b.isLeaf) return a.isLeaf ? 1 : -1;
                return a.name.localeCompare(b.name);
              })
              .map(child => renderTreeNode(child, currentPath, level + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div
      className="fixed z-50 w-80 bg-[#1e1e1e] text-[#cccccc] rounded shadow-2xl overflow-hidden border border-[#333333] flex flex-col menu-container animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {/* 搜索栏 */}
      <div className="flex items-center px-3 py-2 bg-[#252526] border-b border-[#333333]">
        <VscSearch className="text-gray-500 mr-2" size={14} />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search nodes..."
          className="w-full bg-transparent outline-none text-xs"
        />
      </div>

      <div className="max-h-96 overflow-y-auto py-1 custom-scrollbar">
        {query ? (
          searchResults?.length ? (
            searchResults.map((item, idx) => (
              <div
                key={`${item.type}-${idx}`}
                className="px-3 py-1.5 hover:bg-[#2a2d2e] cursor-pointer group flex flex-col"
                onClick={() => onSelect(item)}
              >
                <div className="text-xs font-medium group-hover:text-[var(--accent-color)]">{item.title}</div>
                <div className="text-[9px] text-gray-500 opacity-60">
                  {item.category.join(' > ')}
                </div>
              </div>
            ))
          ) : (
            <div className="px-4 py-8 text-center text-xs text-gray-600 italic">No matches found</div>
          )
        ) : (
          Object.values(root.tree.children)
            .sort((a, b) => {
              if (a.isLeaf !== b.isLeaf) return a.isLeaf ? 1 : -1;
              return a.name.localeCompare(b.name);
            })
            .map(child => renderTreeNode(child, "", 0))
        )}
      </div>
    </div>
  );
}

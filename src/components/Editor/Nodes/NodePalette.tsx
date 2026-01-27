import { useState, useMemo } from "react";
import { NODE_REGISTRY } from "./registry";
import { Pin, BaseNode } from "../Types/nodes";
import { VariableDefinition } from "../Types/variables";

export interface PaletteItem {
  type: string;
  title: string;
  category: string;
  overrides?: Partial<BaseNode>;
}

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

  const filtered = useMemo(() => {
    const allDefs = NODE_REGISTRY.getAllDefinitions();
    const items: PaletteItem[] = [];

    // 1. 处理注册表中的节点
    allDefs.forEach((node) => {
      // 排除通用变量和子图调用节点
      if (
        node.node_type === 'get_variable' ||
        node.node_type === 'set_variable' ||
        node.node_type === 'call_function' ||
        node.node_type === 'call_macro'
      ) {
        return;
      }

      // 基本搜索和类别过滤
      const matchesQuery = node.title.toLowerCase().includes(query.toLowerCase());
      if (!matchesQuery) return;

      // 如果有 filterPin，进一步筛选具有匹配引脚的节点
      if (filterPin) {
        const targetDirection = filterPin.direction === "input" ? "outputs" : "inputs";
        const pins = node[targetDirection] || [];
        const hasCompatiblePin = pins.some(p => p.type === filterPin.type || p.type === 'any');
        if (!hasCompatiblePin) return;
      }

      items.push({
        type: node.node_type,
        title: node.title,
        category: node.category,
      });
    });

    // 2. 处理变量
    const allVars = { ...globalVariables, ...variables };
    Object.values(allVars).forEach((v) => {
      const varName = v.name;
      const varId = v.id;
      const varType = v.data_type;

      // 生成 Get 节点项
      const getTitle = `Get ${varName}`;
      if (getTitle.toLowerCase().includes(query.toLowerCase())) {
        // 兼容过滤逻辑 (Get 节点只有一个输出引脚)
        let compatible = true;
        if (filterPin) {
          if (filterPin.direction === 'output') compatible = false; // Get 节点没有输入引脚 (除了可能有的 exec)
          else {
            compatible = (varType === filterPin.type || filterPin.type === 'any');
          }
        }

        if (compatible) {
          items.push({
            type: 'get_variable',
            title: getTitle,
            category: 'Variables',
            overrides: {
              title: getTitle,
              variableId: varId,
              variableName: varName,
              variableType: varType,
            } as any
          });
        }
      }

      // 生成 Set 节点项
      const setTitle = `Set ${varName}`;
      if (setTitle.toLowerCase().includes(query.toLowerCase())) {
        // 兼容过滤逻辑 (Set 节点有一个输入引脚)
        let compatible = true;
        if (filterPin) {
          if (filterPin.direction === 'input') compatible = false; // Set 只有输入引脚
          else {
            compatible = (varType === filterPin.type || filterPin.type === 'any');
          }
        }

        if (compatible) {
          items.push({
            type: 'set_variable',
            title: setTitle,
            category: 'Variables',
            overrides: {
              title: setTitle,
              variableId: varId,
              variableName: varName,
              variableType: varType,
            } as any
          });
        }
      }
    });

    // 3. 处理函数和宏
    const processSubGraphs = (collection: Record<string, import("../Types/canvas").SubGraphData>, type: 'function' | 'macro') => {
      Object.values(collection).forEach(sub => {
        const title = `${type === 'function' ? 'Call' : 'Macro'} ${sub.name}`;
        if (!title.toLowerCase().includes(query.toLowerCase())) return;

        // 兼容过滤逻辑 (简单检查是否有匹配的引脚类型)
        if (filterPin) {
          const targetPins = filterPin.direction === 'input' ? sub.outputs : sub.inputs;
          const hasCompatible = (targetPins || []).some(p => p.type === filterPin.type || p.type === 'any' || filterPin.type === 'any');
          // 同时检查执行流引脚
          const hasExec = filterPin.type === 'exec';
          if (!hasCompatible && !hasExec) return;
        }

        items.push({
          type: type === 'function' ? 'call_function' : 'call_macro',
          title,
          category: type === 'function' ? 'Functions' : 'Macros',
          overrides: {
            subGraphId: sub.id,
            title: sub.name, // 节点内部显示的标题通常不带 "Call"
          } as any
        });
      });
    };

    processSubGraphs(functions, 'function');
    processSubGraphs(macros, 'macro');

    return items;
  }, [query, filterPin, variables, globalVariables, functions, macros]);

  return (
    <div
      className="fixed z-50 w-64 bg-gray-800 text-white rounded shadow-lg overflow-hidden border border-gray-700 menu-container"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search nodes..."
        className="w-full px-3 py-2 text-sm bg-gray-700 outline-none border-b border-gray-600"
      />

      <div className="max-h-64 overflow-y-auto">
        {filtered.map((item, idx) => (
          <div
            key={`${item.type}-${item.title}-${idx}`}
            className="px-3 py-2 hover:bg-gray-600 cursor-pointer flex justify-between items-center transition-colors"
            onClick={() => onSelect(item)}
          >
            <div>
              <div className="font-medium text-sm">{item.title}</div>
              <div className="text-[10px] text-gray-400 uppercase font-mono">{item.category}</div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

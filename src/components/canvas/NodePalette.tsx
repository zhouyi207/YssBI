import { useState, useMemo } from "react";
import { NODE_REGISTRY } from "../node/registry";
import { Pin } from "../node/models";

export default function NodePalette({
  x,
  y,
  onSelect,
  filterPin,
}: {
  x: number;
  y: number;
  onSelect: (tpl: { type: string }) => void;
  filterPin?: Pin | null;
}) {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const allDefs = NODE_REGISTRY.getAllDefinitions();
    return allDefs.filter((node) => {
      // 基本搜索和类别过滤
      if (node.category === "Variable") return false;
      const matchesQuery = node.title.toLowerCase().includes(query.toLowerCase());
      if (!matchesQuery) return false;

      // 如果有 filterPin，进一步筛选具有匹配引脚的节点
      if (filterPin) {
        const targetDirection = filterPin.direction === "input" ? "outputs" : "inputs";
        const pins = node[targetDirection] || [];
        const hasCompatiblePin = pins.some(p => p.type === filterPin.type);
        if (!hasCompatiblePin) return false;
      }

      return true;
    });
  }, [query, filterPin]);

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
        {filtered.map((node) => (
          <div
            key={node.node_type}
            className="px-3 py-2 hover:bg-gray-600 cursor-pointer flex justify-between items-center transition-colors"
            onClick={() => onSelect({ type: node.node_type })}
          >
            <div>
              <div className="font-medium text-sm">{node.title}</div>
              <div className="text-[10px] text-gray-400 uppercase font-mono">{node.category}</div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

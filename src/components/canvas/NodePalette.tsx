import { useState, useMemo } from "react";
import { NODE_REGISTRY } from "../node/registry";

export default function NodePalette({
  x,
  y,
  onSelect,
}: {
  x: number;
  y: number;
  onSelect: (tpl: { type: string }) => void;
}) {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    return Object.entries(NODE_REGISTRY).filter(([_type, node]) =>
      node.category !== "Variable" &&
      node.title.toLowerCase().includes(query.toLowerCase())
    );
  }, [query]);

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
        {filtered.map(([type, node]) => (
          <div
            key={type}
            className="px-3 py-2 hover:bg-gray-600 cursor-pointer flex justify-between items-center transition-colors"
            onClick={() => onSelect({ type })}
          >
            <div>
              <div className="font-medium text-sm">{node.title}</div>
              <div className="text-[10px] text-gray-400 uppercase font-mono">{node.category}</div>
            </div>
            {node.ui?.icon && <span className="text-lg opacity-80">{node.ui.icon}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

import { useDrag } from "./drag/DragContext";
import { NODE_REGISTRY } from "./node/registry";

const PIN_COLORS: Record<string, string> = {
  exec: "#ffffff",
  int: "#3b82f6",
  float: "#3b82f6",
  bool: "#f64146",
  string: "#10b981",
  object: "#8b5cf6",
  array: "#ef4444",
  struct: "#f97316",
  delegate: "#ec4899",
};

export default function Sidebar() {
  const { startDrag } = useDrag();

  // 过滤出唯一变量（每个类型只显示一个，比如 int 类型只显示一个变量入口）
  const uniqueVariableTypes = Object.entries(NODE_REGISTRY).reduce((acc, [type, node]) => {
    if (node.category === "Variable") {
      const varType = node.initialOutputs?.[0]?.type || node.initialInputs?.[0]?.type || "int";
      if (!acc.find(item => item.varType === varType)) {
        acc.push({ type, node, varType });
      }
    }
    return acc;
  }, [] as { type: string, node: any, varType: string }[]);

  return (
    <div className="w-56 border-r bg-white flex flex-col h-full overflow-hidden shadow-sm">
      {/* Header */}
      <div className="p-3 border-b bg-gray-50/50 flex justify-between items-center">
        <span className="text-[11px] font-black text-gray-500 uppercase tracking-widest">
          Variables
        </span>
        <button className="text-gray-400 hover:text-blue-500 transition-colors">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </button>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto p-1.5 space-y-1">
      {uniqueVariableTypes.length > 0 ? (
        uniqueVariableTypes.map(({ type, varType }) => {
            const typeColor = PIN_COLORS[varType as keyof typeof PIN_COLORS];

            return (
              <div
                key={type}
                onPointerDown={(e) => {
                  e.preventDefault();
                  startDrag({
                    type: "node-template",
                    template: { type, category: "Variable" }, // 标记为变量类别
                    x: e.clientX,
                    y: e.clientY,
                    startX: e.clientX,
                    startY: e.clientY,
                  });
                }}
                className="
                  group flex items-center gap-2 p-2 rounded-md cursor-grab
                  hover:bg-blue-50 active:bg-blue-100 transition-all
                  border border-transparent hover:border-blue-100
                "
              >
                {/* Type Badge */}
                <div 
                  className="w-8 h-4 shrink-0 rounded-[3px] flex items-center justify-center text-[9px] font-black text-white uppercase"
                  style={{ backgroundColor: typeColor }}
                >
                  {varType}
                </div>

                {/* Name */}
                <div className="flex-1 min-w-0">
                  <div className="text-[12px] font-bold text-gray-700 truncate">
                    Variable
                  </div>
                </div>
              </div>
            );
          })
        ) : (
          <div className="p-8 text-center">
            <div className="text-[11px] text-gray-400 italic">No variables yet</div>
          </div>
        )}
      </div>
    </div>
  );
}

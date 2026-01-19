import { useState, useEffect } from "react";
import { useDrag } from "./drag/DragContext";
import { useCanvas } from "./canvas/CanvasContext";
import { useUI } from "./ui/UIProvider";


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
  const { 
    variables, 
    selectedVariableId, 
    setSelectedVariableId, 
    updateVariable, 
    addVariable, 
    deleteVariable 
  } = useCanvas();

  const { showDialog } = useUI();

  const [isAdding, setIsAdding] = useState(false);
  const [newVarName, setNewVarName] = useState("");
  const [newVarType, setNewVarType] = useState("int");

  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    variableId: string;
  } | null>(null);

  // 点击外部关闭右键菜单
  useEffect(() => {
    const hideMenu = () => setContextMenu(null);
    window.addEventListener("click", hideMenu);
    window.addEventListener("contextmenu", hideMenu);
    return () => {
      window.removeEventListener("click", hideMenu);
      window.removeEventListener("contextmenu", hideMenu);
    };
  }, []);

  const handleAddVariable = () => {
    if (newVarName.trim()) {
      addVariable(newVarName.trim(), newVarType);
      setNewVarName("");
      setIsAdding(false);
    }
  };

  const handleRename = (id: string) => {
    setRenamingId(id);
    setEditName(variables[id].name);
  };

  const confirmRename = () => {
    if (renamingId && editName.trim()) {
      if (editName.trim() !== variables[renamingId].name) {
        updateVariable(renamingId, { name: editName.trim() });
      }
    }
    setRenamingId(null);
  };

  // 过滤出通用节点模板（排除变量类别，因为变量现在有专门的管理器）
  // const nodeTemplates = NODE_REGISTRY.getAllDefinitions().filter((node) => node.category !== "Variable");

  const selectedVar = selectedVariableId ? variables[selectedVariableId] : null;

  return (
    <div 
      className="sidebar-container w-64 border-r bg-white flex flex-col h-full overflow-hidden shadow-sm select-none"
      onWheel={(e) => e.stopPropagation()}
    >
      {/* Variables Manager Section */}
      <div className="flex-1 flex flex-col min-h-0 relative">
        <div className="p-3 border-b bg-gray-100/50 flex justify-between items-center">
          <span className="text-[11px] font-black text-gray-500 uppercase tracking-widest">
            Variables
          </span>
          <button 
            onClick={() => setIsAdding(!isAdding)}
            className={`transition-colors ${isAdding ? 'text-red-500' : 'text-gray-400 hover:text-blue-600'}`}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              {isAdding ? <path d="M18 6L6 18M6 6l12 12" /> : <path d="M12 5v14M5 12h14" />}
            </svg>
          </button>
        </div>

        {/* Add Variable Form */}
        {isAdding && (
          <div className="p-3 bg-blue-50 border-b border-blue-100 space-y-2">
            <input 
              autoFocus
              className="w-full text-xs p-1.5 rounded border border-blue-200 focus:outline-none focus:ring-2 focus:ring-blue-400"
              placeholder="Variable Name..."
              value={newVarName}
              onChange={e => setNewVarName(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleAddVariable()}
            />
            <div className="flex gap-2">
              <select 
                className="flex-1 text-[10px] p-1 rounded border border-blue-200 bg-white"
                value={newVarType}
                onChange={e => setNewVarType(e.target.value)}
              >
                <option value="int">Int</option>
                <option value="float">Float</option>
                <option value="bool">Bool</option>
                <option value="string">String</option>
              </select>
              <button 
                onClick={handleAddVariable}
                className="px-3 py-1 bg-blue-600 text-white text-[10px] font-bold rounded hover:bg-blue-700"
              >
                Add
              </button>
            </div>
          </div>
        )}
        
        {/* Variables List */}
        <div className="flex-1 overflow-y-auto p-1.5 space-y-1 border-b">
          {Object.entries(variables).map(([id, data]) => (
            <div 
              key={id} 
              onClick={() => setSelectedVariableId(id)}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setContextMenu({ x: e.clientX, y: e.clientY, variableId: id });
              }}
              onPointerDown={(e) => {
                if (e.button !== 0) return; // 仅左键拖拽
                e.preventDefault();
                startDrag({
                  type: "node-template",
                  template: { 
                    type: "get_variable", // 默认拖拽出 Get 节点
                    category: "Variable",
                    variableId: id,
                    variableName: data.name,
                    variableType: data.type
                  },
                  x: e.clientX,
                  y: e.clientY,
                  startX: e.clientX,
                  startY: e.clientY,
                });
              }}
              className={`
                group flex items-center gap-2 p-2 rounded cursor-grab transition-all
                ${selectedVariableId === id 
                  ? 'bg-blue-600 text-white shadow-md' 
                  : 'hover:bg-white text-gray-700 border border-transparent hover:border-gray-200'}
              `}
            >
              <div 
                className="w-2 h-2 rounded-full shrink-0"
                style={{ backgroundColor: selectedVariableId === id ? 'white' : PIN_COLORS[data.type] }}
              />
              {renamingId === id ? (
                <input
                  autoFocus
                  className="flex-1 bg-white/20 text-white border-none outline-none px-1 rounded text-[12px] font-bold"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  onBlur={confirmRename}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") confirmRename();
                    if (e.key === "Escape") setRenamingId(null);
                  }}
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <span className="flex-1 text-[12px] font-bold truncate">{data.name}</span>
              )}
              <span className={`text-[9px] font-black uppercase px-1 rounded ${selectedVariableId === id ? 'bg-blue-500' : 'bg-gray-200 text-gray-500'}`}>
                {data.type}
              </span>
            </div>
          ))}
          {Object.keys(variables).length === 0 && (
            <div className="text-center py-8 text-[11px] text-gray-400 italic">No variables yet</div>
          )}
        </div>

        {/* Variable Context Menu */}
        {contextMenu && (
          <div 
            className="fixed z-[100] bg-gray-800 border border-gray-700 rounded shadow-xl py-1 w-32"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onClick={(e) => e.stopPropagation()}
          >
            <div 
              onClick={() => { handleRename(contextMenu.variableId); setContextMenu(null); }}
              className="px-3 py-1.5 text-xs text-gray-300 hover:bg-blue-600 hover:text-white cursor-pointer flex items-center gap-2"
            >
              <span>Rename</span>
            </div>
            <div 
              onClick={() => {
                showDialog({
                  title: "Delete Variable",
                  message: `Are you sure you want to delete variable '${variables[contextMenu.variableId].name}'?`,
                  type: "danger",
                  confirmText: "Delete",
                  onConfirm: () => {
                    deleteVariable(contextMenu.variableId);
                    if (selectedVariableId === contextMenu.variableId) setSelectedVariableId(null);
                  }
                });
                setContextMenu(null);
              }}
              className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-600 hover:text-white cursor-pointer flex items-center gap-2"
            >
              <span>Delete</span>
            </div>
          </div>
        )}

        {/* 2. Properties Window */}
        <div className="h-[45%] bg-white flex flex-col overflow-hidden">
          <div className="p-2 border-b bg-gray-50">
            <span className="text-[10px] font-black text-gray-400 uppercase tracking-tighter">
              Details {selectedVar ? `: ${selectedVar.name}` : ''}
            </span>
          </div>
          
          {selectedVar ? (
            <div className="flex-1 overflow-y-auto">
              <table className="w-full text-[11px] border-collapse">
                <tbody>
                  <tr className="border-b">
                    <td className="p-2 font-bold text-gray-500 bg-gray-50/50 w-20">Name</td>
                    <td className="p-2">
                      <input 
                        className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                        value={selectedVar.name}
                        onChange={(e) => updateVariable(selectedVariableId!, { name: e.target.value })}
                      />
                    </td>
                  </tr>
                  <tr className="border-b">
                    <td className="p-2 font-bold text-gray-500 bg-gray-50/50">Type</td>
                    <td className="p-2">
                      <select 
                        className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                        value={selectedVar.type}
                        onChange={(e) => updateVariable(selectedVariableId!, { type: e.target.value })}
                      >
                        <option value="int">Integer</option>
                        <option value="float">Float</option>
                        <option value="bool">Boolean</option>
                        <option value="string">String</option>
                      </select>
                    </td>
                  </tr>
                  <tr className="border-b">
                    <td className="p-2 font-bold text-gray-500 bg-gray-50/50">Value</td>
                    <td className="p-2">
                      {selectedVar.type === "bool" ? (
                        <input 
                          type="checkbox"
                          className="rounded text-blue-600 focus:ring-blue-500"
                          checked={!!selectedVar.value}
                          onChange={(e) => updateVariable(selectedVariableId!, { value: e.target.checked })}
                        />
                      ) : (
                        <input 
                          className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                          type={selectedVar.type === "string" ? "text" : "number"}
                          value={selectedVar.value}
                          onChange={(e) => {
                            const val = selectedVar.type === "string" ? e.target.value : Number(e.target.value);
                            updateVariable(selectedVariableId!, { value: val });
                          }}
                        />
                      )}
                    </td>
                  </tr>
                  <tr>
                    <td className="p-2" colSpan={2}>
                      <button 
                        onClick={() => {
                          const idToDelete = selectedVariableId!;
                          showDialog({
                            title: "Delete Variable",
                            message: `Are you sure you want to delete variable '${selectedVar.name}'?`,
                            type: "danger",
                            confirmText: "Delete",
                            onConfirm: () => {
                              deleteVariable(idToDelete);
                              setSelectedVariableId(null);
                            }
                          });
                        }}
                        className="w-full py-1.5 mt-2 border border-red-100 text-red-500 hover:bg-red-50 rounded transition-colors font-bold text-[10px]"
                      >
                        DELETE VARIABLE
                      </button>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center p-4 text-center">
              <span className="text-[11px] text-gray-400 italic leading-tight">
                Select a variable from the list above to edit its properties
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

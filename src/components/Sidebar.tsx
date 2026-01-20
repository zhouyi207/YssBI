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

interface SectionProps {
  title: string;
  isOpen: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  children: React.ReactNode;
}

const Section: React.FC<SectionProps> = ({ title, isOpen, onToggle, onAdd, children }) => (
  <div className="border-b border-gray-200">
    <div 
      className="flex items-center justify-between p-2 bg-gray-50/50 hover:bg-gray-100 cursor-pointer transition-colors group"
      onClick={onToggle}
    >
      <div className="flex items-center gap-2">
        <svg 
          width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
          className={`transition-transform duration-200 ${isOpen ? 'rotate-90' : ''}`}
        >
          <path d="M9 18l6-6-6-6" />
        </svg>
        <span className="text-[11px] font-black text-gray-500 uppercase tracking-widest select-none">
          {title}
        </span>
      </div>
      {onAdd && (
        <button 
          onClick={(e) => { e.stopPropagation(); onAdd(); }}
          className="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-gray-400 hover:text-blue-600"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </button>
      )}
    </div>
    {isOpen && <div className="p-1">{children}</div>}
  </div>
);

export default function Sidebar() {
  const { startDrag } = useDrag();
  const { 
    variables, 
    globalVariables,
    selectedVariableId, 
    setSelectedVariableId, 
    addVariable, 
    deleteVariable,
    promoteVariable,
    demoteVariable,
    functions,
    addFunction,
    deleteFunction,
    macros,
    addMacro,
    deleteMacro
  } = useCanvas();

  const { showDialog } = useUI();

  // Collapsible states
  const [sections, setSections] = useState({
    variables: true,
    functions: true,
    macros: true
  });

  const toggleSection = (name: keyof typeof sections) => {
    setSections(prev => ({ ...prev, [name]: !prev[name] }));
  };

  // Add Item states
  const [addingType, setAddingType] = useState<'variable' | 'function' | 'macro' | null>(null);
  const [newName, setNewName] = useState("");
  const [newVarType, setNewVarType] = useState("int");
  const [newVarIsGlobal, setNewVarIsGlobal] = useState(false);

  const handleAdd = () => {
    if (!newName.trim()) return;
    if (addingType === 'variable') {
      addVariable(newName.trim(), newVarType, newVarIsGlobal);
    } else if (addingType === 'function') {
      addFunction(newName.trim());
    } else if (addingType === 'macro') {
      addMacro(newName.trim());
    }
    setNewName("");
    setAddingType(null);
  };

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    type: 'variable' | 'function' | 'macro';
    id: string;
  } | null>(null);

  useEffect(() => {
    const hideMenu = () => setContextMenu(null);
    window.addEventListener("click", hideMenu);
    window.addEventListener("contextmenu", hideMenu);
    return () => {
      window.removeEventListener("click", hideMenu);
      window.removeEventListener("contextmenu", hideMenu);
    };
  }, []);

  const renderItem = (id: string, name: string, type: 'variable' | 'function' | 'macro', extra?: any) => {
    const isSelected = selectedVariableId === id; // For now only variables are selectable for details
    
    return (
      <div 
        key={id} 
        onClick={() => type === 'variable' && setSelectedVariableId(id)}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setContextMenu({ x: e.clientX, y: e.clientY, type, id });
        }}
        onPointerDown={(e) => {
          if (e.button !== 0) return; 
          if ((e.target as HTMLElement).closest('button')) return;
          if (type === 'variable') {
            e.preventDefault();
            startDrag({
              type: "node-template",
              template: { 
                type: "get_variable",
                category: "Variable",
                variableId: id,
                variableName: name,
                variableType: extra?.type
              },
              x: e.clientX,
              y: e.clientY,
              startX: e.clientX,
              startY: e.clientY,
            });
          }
        }}
        className={`
          group flex items-center gap-2 p-1.5 rounded cursor-grab transition-all
          ${isSelected 
            ? 'bg-blue-600 text-white shadow-md' 
            : 'hover:bg-gray-100 text-gray-700 border border-transparent'}
        `}
      >
        <div 
          className="w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: isSelected ? 'white' : (extra?.type ? PIN_COLORS[extra.type] : '#9ca3af') }}
        />
        <span className="flex-1 text-[12px] font-bold truncate">{name}</span>
        
        {type === 'variable' && (
          <>
            {!extra?.isGlobal ? (
              <button
                onClick={(e) => { e.stopPropagation(); promoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="提升为全局变量"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
              </button>
            ) : (
              <button
                onClick={(e) => { e.stopPropagation(); demoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="降级为局部变量"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.45 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                  <line x1="1" y1="1" x2="23" y2="23" />
                </svg>
              </button>
            )}
            <span className={`text-[9px] font-black uppercase px-1 rounded ${isSelected ? 'bg-blue-500' : 'bg-gray-200 text-gray-500'}`}>
              {extra?.type}
            </span>
          </>
        )}
      </div>
    );
  };

  return (
    <div 
      className="sidebar-container w-64 border-r bg-white flex flex-col h-full overflow-hidden shadow-sm select-none"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="flex-1 overflow-y-auto min-h-0 relative">
        {/* Sections */}
        <Section 
          title="Functions" 
          isOpen={sections.functions} 
          onToggle={() => toggleSection('functions')}
          onAdd={() => setAddingType('function')}
        >
          {Object.entries(functions).map(([id, data]) => renderItem(id, data.name, 'function'))}
          {Object.keys(functions).length === 0 && <div className="text-[10px] text-gray-400 italic p-2">No functions</div>}
        </Section>

        <Section 
          title="Macros" 
          isOpen={sections.macros} 
          onToggle={() => toggleSection('macros')}
          onAdd={() => setAddingType('macro')}
        >
          {Object.entries(macros).map(([id, data]) => renderItem(id, data.name, 'macro'))}
          {Object.keys(macros).length === 0 && <div className="text-[10px] text-gray-400 italic p-2">No macros</div>}
        </Section>

        <Section 
          title="Variables" 
          isOpen={sections.variables} 
          onToggle={() => toggleSection('variables')}
          onAdd={() => setAddingType('variable')}
        >
          {/* Global */}
          {Object.keys(globalVariables).length > 0 && (
            <div className="mb-2">
              <div className="px-2 py-1 text-[8px] font-black text-gray-400 uppercase tracking-tighter flex items-center gap-2">
                Global
                <div className="h-px flex-1 bg-gray-100" />
              </div>
              {Object.entries(globalVariables).map(([id, data]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: true }))}
            </div>
          )}
          {/* Local */}
          <div>
            {Object.keys(globalVariables).length > 0 && (
              <div className="px-2 py-1 text-[8px] font-black text-gray-400 uppercase tracking-tighter flex items-center gap-2">
                Local
                <div className="h-px flex-1 bg-gray-100" />
              </div>
            )}
            {Object.entries(variables).map(([id, data]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: false }))}
          </div>
          {Object.keys(variables).length === 0 && Object.keys(globalVariables).length === 0 && (
            <div className="text-[10px] text-gray-400 italic p-2">No variables</div>
          )}
        </Section>

        {/* Add Popup UI */}
        {addingType && (
          <div className="p-3 bg-blue-50 border-b border-blue-100 space-y-2">
            <div className="flex justify-between items-center mb-1">
              <span className="text-[10px] font-bold text-blue-600 uppercase">Add {addingType}</span>
              <button onClick={() => setAddingType(null)} className="text-gray-400 hover:text-red-500">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
            <input 
              autoFocus
              className="w-full text-xs p-1.5 rounded border border-blue-200 focus:outline-none focus:ring-2 focus:ring-blue-400"
              placeholder={`${addingType.charAt(0).toUpperCase() + addingType.slice(1)} Name...`}
              value={newName}
              onChange={e => setNewName(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleAdd()}
            />
            {addingType === 'variable' && (
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
                <div className="flex items-center gap-1 bg-white border border-blue-200 rounded px-1.5">
                  <input 
                    type="checkbox" 
                    id="global-check" 
                    checked={newVarIsGlobal} 
                    onChange={e => setNewVarIsGlobal(e.target.checked)}
                    className="w-3 h-3"
                  />
                  <label htmlFor="global-check" className="text-[9px] font-bold text-gray-500 cursor-pointer">GLOBAL</label>
                </div>
              </div>
            )}
            <button 
              onClick={handleAdd}
              className="w-full py-1.5 bg-blue-600 text-white text-[10px] font-bold rounded hover:bg-blue-700"
            >
              Confirm
            </button>
          </div>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <div 
          className="fixed z-[100] bg-gray-800 border border-gray-700 rounded shadow-xl py-1 w-32"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <div 
            onClick={() => {
              const name = contextMenu.type === 'variable' 
                ? (variables[contextMenu.id]?.name || globalVariables[contextMenu.id]?.name)
                : (contextMenu.type === 'function' ? functions[contextMenu.id]?.name : macros[contextMenu.id]?.name);
              
              showDialog({
                title: `Delete ${contextMenu.type}`,
                message: `Are you sure you want to delete ${contextMenu.type} '${name}'?`,
                type: "danger",
                confirmText: "Delete",
                onConfirm: () => {
                  if (contextMenu.type === 'variable') {
                    deleteVariable(contextMenu.id);
                    if (selectedVariableId === contextMenu.id) setSelectedVariableId(null);
                  } else if (contextMenu.type === 'function') {
                    deleteFunction(contextMenu.id);
                  } else if (contextMenu.type === 'macro') {
                    deleteMacro(contextMenu.id);
                  }
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
    </div>
  );
}

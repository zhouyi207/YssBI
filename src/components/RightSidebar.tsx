import React from "react";
import { useCanvas } from "./canvas/CanvasContext";
import { useUI } from "./ui/UIProvider";

export const RightSidebar: React.FC = () => {
  const { 
    variables, 
    globalVariables,
    selectedVariableId, 
    setSelectedVariableId, 
    updateVariable, 
    deleteVariable 
  } = useCanvas();
  const { showDialog } = useUI();

  const selectedVar = selectedVariableId ? (variables[selectedVariableId] || globalVariables[selectedVariableId]) : null;

  return (
    <div 
      className="right-sidebar-container w-64 border-l bg-white flex flex-col h-full overflow-hidden shadow-sm select-none"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="p-2 border-b bg-gray-50 flex justify-between items-center h-10 shrink-0">
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
            Select an item to edit its properties
          </span>
        </div>
      )}
    </div>
  );
};

import React from "react";
import { useCanvas } from "../Context/CanvasContext";
import { useUI } from "../Context/UIProvider";
import { Select } from "../Shared/UI/Select";

export const RightSidebar: React.FC = () => {
  const {
    variables,
    globalVariables,
    events,
    functions,
    macros,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    updateVariable,
    deleteVariable,
    updateEvent,
    deleteEvent,
    updateFunction,
    deleteFunction,
    updateMacro,
    deleteMacro
  } = useCanvas();
  const { showDialog } = useUI();

  // Find the selected item's data
  let selectedData: any = null;
  if (selectedItemId && selectedItemType) {
    if (selectedItemType === 'variable') {
      selectedData = variables[selectedItemId] || globalVariables[selectedItemId];
    } else if (selectedItemType === 'event') {
      selectedData = events[selectedItemId];
    } else if (selectedItemType === 'function') {
      selectedData = functions[selectedItemId];
    } else if (selectedItemType === 'macro') {
      selectedData = macros[selectedItemId];
    }
  }


  const handleUpdate = (data: any) => {
    if (!selectedItemId || !selectedItemType) return;
    if (selectedItemType === 'variable') updateVariable(selectedItemId, data);
    else if (selectedItemType === 'event') updateEvent(selectedItemId, data);
    else if (selectedItemType === 'function') updateFunction(selectedItemId, data);
    else if (selectedItemType === 'macro') updateMacro(selectedItemId, data);
  };
  const handleDelete = () => {
    if (!selectedItemId || !selectedItemType) return;
    showDialog({
      title: `Delete ${selectedItemType}`,
      message: `Are you sure you want to delete ${selectedItemType} '${selectedData.name}'?`,
      type: "danger",
      confirmText: "Delete",
      onConfirm: () => {
        if (selectedItemType === 'variable') deleteVariable(selectedItemId);
        else if (selectedItemType === 'event') deleteEvent(selectedItemId);
        else if (selectedItemType === 'function') deleteFunction(selectedItemId);
        else if (selectedItemType === 'macro') deleteMacro(selectedItemId);
        setSelectedInfo(null, null);
      }
    });
  };
  const renderPinEditor = (title: string, pins: any[] = [], isInput: boolean) => {
    return (
      <div className="mt-4 px-2">
        <div className="flex justify-between items-center mb-1">
          <span className="text-[10px] font-black text-gray-400 uppercase">{title}</span>
          <button
            onClick={() => {
              const newPins = [...pins, { id: `pin-${crypto.randomUUID()}`, name: "NewPin", type: "int" }];
              handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
            }}
            className="p-1 hover:bg-white/10 rounded text-gray-400 hover:text-[var(--accent-color)] transition-colors"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </button>
        </div>
        <div className="space-y-1">
          {pins.map((pin, idx) => (
            <div key={pin.id} className="flex gap-1 items-center bg-white/5 p-1 rounded group">
              <input
                className="flex-1 bg-transparent border-none text-[10px] focus:ring-0 p-0 font-medium"
                value={pin.name}
                onChange={(e) => {
                  const newPins = [...pins];
                  newPins[idx] = { ...newPins[idx], name: e.target.value };
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
              />
              <Select
                className="w-24"
                value={pin.type}
                options={["exec", "int", "float", "bool", "string", "object"]}
                onChange={(val) => {
                  const newPins = [...pins];
                  newPins[idx] = { ...newPins[idx], type: val };
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
              />
              <button
                onClick={() => {
                  const newPins = pins.filter((_, i) => i !== idx);
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
                className="opacity-0 group-hover:opacity-100 p-0.5 hover:text-red-500 transition-all"
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
          ))}
          {pins.length === 0 && <div className="text-[9px] text-gray-300 italic text-center py-1">No {title.toLowerCase()}</div>}
        </div>
      </div>
    );
  };

  return (
    <div
      className="right-sidebar-container w-64 border-l border-[#2b2b2b] bg-[var(--sidebar-bg)] flex flex-col h-full overflow-hidden shadow-sm select-none"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="px-3 border-b border-[#2b2b2b] bg-[var(--workbench-bg)]/50 flex justify-between items-center h-9 shrink-0">
        <span className="text-[10px] font-black text-gray-400 font-mono tracking-tighter uppercase">
          Details {selectedData ? `: ${selectedData.name}` : ''}
        </span>
      </div>

      {selectedData ? (
        <div className="flex-1 overflow-y-auto pb-4">
          <table className="w-full text-[11px] border-collapse text-[#cccccc]">
            <tbody>
              <tr className="border-b border-[#2b2b2b]">
                <td className="p-2 font-bold text-gray-400 bg-white/5 w-20">Name</td>
                <td className="p-2">
                  <input
                    className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                    value={selectedData.name}
                    onChange={(e) => handleUpdate({ name: e.target.value })}
                  />
                </td>
              </tr>
              {selectedItemType === 'variable' && (
                <>
                  <tr className="border-b border-[#2b2b2b]">
                    <td className="p-2 font-bold text-gray-400 bg-white/5">Type</td>
                    <td className="p-2">
                      <Select
                        value={selectedData.type}
                        options={[
                          { label: "Integer", value: "int" },
                          { label: "Float", value: "float" },
                          { label: "Boolean", value: "bool" },
                          { label: "String", value: "string" }
                        ]}
                        onChange={(val) => handleUpdate({ type: val })}
                      />
                    </td>
                  </tr>
                  <tr className="border-b border-[#2b2b2b]">
                    <td className="p-2 font-bold text-gray-400 bg-white/5">Value</td>
                    <td className="p-2">
                      {selectedData.type === "bool" ? (
                        <input
                          type="checkbox"
                          className="rounded text-[var(--accent-color)] focus:ring-[var(--accent-color)] bg-transparent border-[#2b2b2b]"
                          checked={!!selectedData.value}
                          onChange={(e) => handleUpdate({ value: e.target.checked })}
                        />
                      ) : (
                        <input
                          className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                          type={selectedData.type === "string" ? "text" : "number"}
                          value={selectedData.value}
                          onChange={(e) => {
                            const val = selectedData.type === "string" ? e.target.value : Number(e.target.value);
                            handleUpdate({ value: val });
                          }}
                        />
                      )}
                    </td>
                  </tr>
                </>
              )}
              {(selectedItemType === 'function' || selectedItemType === 'macro') && (
                <tr className="border-b border-[#2b2b2b]">
                  <td className="p-2 font-bold text-gray-400 bg-white/5">Type</td>
                  <td className="p-2 text-gray-400 italic">
                    {selectedItemType.charAt(0).toUpperCase() + selectedItemType.slice(1)}
                  </td>
                </tr>
              )}

            </tbody>
          </table>

          {(selectedItemType === 'function' || selectedItemType === 'macro') && (
            <>
              {renderPinEditor("Inputs", selectedData.inputs, true)}
              {renderPinEditor("Outputs", selectedData.outputs, false)}
            </>
          )}
          <div className="p-2">
            <button
              onClick={handleDelete}
              className="w-full py-1.5 mt-4 border border-red-500/30 text-red-500 hover:bg-red-500/10 rounded transition-colors font-bold text-[9px] uppercase tracking-wider"
            >
              Delete {selectedItemType}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex-1 flex flex-col items-center justify-center p-4 text-center opacity-30 group">
          <svg className="w-12 h-12 mb-2 text-gray-300 group-hover:scale-110 transition-transform" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
            <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
            <polyline points="13 2 13 9 20 9" />
          </svg>
          <span className="text-[10px] font-bold text-gray-400 uppercase tracking-widest">
            No selection
          </span>
          <span className="text-[9px] text-gray-400 mt-1 italic">
            Select an item from the sidebar to edit
          </span>
        </div>
      )}
    </div>
  );
};

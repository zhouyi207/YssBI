import { forwardRef, useMemo } from "react";
import { useEditorGroup } from "@/features/application/editor";
import { Select } from "@/shared/ui";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { dataTypeKind, dataTypeFromKey, isPrimitiveType } from "@/shared/types/domain/dataType";
import { dataValueToRaw, dataValueFromRaw } from "@/shared/types/domain/dataValue";
import { uiStore } from "@/features/core/ui/UIStore";
import { useLogStore } from "@/features/core/log/logStore";
import { LogLevel, LogType } from "@/shared/types/ui";

const LOG_TYPE_LABELS: Record<string, string> = {
  application: 'APP', execution: 'EXEC', system: 'SYS', graph: 'GRAPH', data: 'DATA',
};

const getLevelColor = (level: LogLevel) => {
  switch (level) {
    case 'error': return 'text-red-400';
    case 'warn': return 'text-yellow-400';
    case 'info': return 'text-blue-400';
    case 'debug': return 'text-gray-400';
    case 'trace': return 'text-gray-500';
    default: return 'text-gray-400';
  }
};

const getTypeColor = (type: LogType) => {
  switch (type) {
    case 'application': return 'text-green-400';
    case 'execution': return 'text-purple-400';
    case 'system': return 'text-cyan-400';
    case 'graph': return 'text-orange-400';
    case 'data': return 'text-pink-400';
    default: return 'text-gray-400';
  }
};

export const Detail = forwardRef<HTMLDivElement, { width?: number }>(({ }, ref) => {
  const {
    Variables,
    events,
    functions,
    dataframes,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    updateVariable,
    deleteVariable,
    updateEvent,
    deleteEvent,
    updateFunction,
    deleteFunction,
    updateDataFrame,
    deleteDataFrame
  } = useEditorGroup();

  const selectedLog = useLogStore((s) => s.selectedLog);


  // Find the selected item's data
  const selectedData = useMemo(() => {
    if (!selectedItemId || !selectedItemType) return null;
    if (selectedItemType === 'variable') {
      return Variables[selectedItemId];
    } else if (selectedItemType === 'event') {
      return events[selectedItemId];
    } else if (selectedItemType === 'function') {
      return functions[selectedItemId];
    } else if (selectedItemType === 'data') {
      return dataframes[selectedItemId];
    }
    return null;
  }, [selectedItemId, selectedItemType, Variables, events, functions, dataframes]);

  const handleUpdate = (data: any) => {
    if (!selectedItemId || !selectedItemType) return;
    if (selectedItemType === 'variable') updateVariable(selectedItemId, data);
    else if (selectedItemType === 'event') updateEvent(selectedItemId, data);
    else if (selectedItemType === 'function') updateFunction(selectedItemId, data);
    else if (selectedItemType === 'data') updateDataFrame(selectedItemId, data);
  };

  const handleDelete = () => {
    if (!selectedItemId || !selectedItemType) return;
    uiStore.showDialog({
      title: `Delete ${selectedItemType}`,
      message: `Are you sure you want to delete ${selectedItemType} '${selectedData.name}'?`,
      type: "danger",
      confirmText: "Delete",
      onConfirm: () => {
        if (selectedItemType === 'variable') deleteVariable(selectedItemId);
        else if (selectedItemType === 'event') deleteEvent(selectedItemId);
        else if (selectedItemType === 'function') deleteFunction(selectedItemId);
        else if (selectedItemType === 'data') deleteDataFrame(selectedItemId);
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
                  const newPins = [...pins];
                  const current = newPins[idx].containerType;
                  const next = current === 'array' ? 'dataseries' : current === 'dataseries' ? undefined : 'array';
                  newPins[idx] = { ...newPins[idx], containerType: next };
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
                className={`p-1 rounded transition-colors ${pin.containerType ? 'text-blue-400 bg-blue-500/10' : 'text-gray-500 hover:bg-white/5'}`}
                title={`Container: ${pin.containerType ?? 'none'} (click to cycle)`}
              >
                <span className="text-[9px] font-black">{pin.containerType === 'dataseries' ? '◇' : pin.containerType === 'array' ? '[]' : '·'}</span>
              </button>
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
      ref={ref}
      className="right-sidebar-container bg-[var(--sidebar-bg)] flex flex-col h-full w-full overflow-hidden select-none"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="px-3 border-b border-[#2b2b2b] bg-[var(--workbench-bg)]/50 flex justify-between items-center shrink-0" style={{ height: 'var(--titlebar-height)' }}>
        <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest">
          Details {selectedItemType === 'log' ? ': Log' : selectedData ? `: ${selectedData.name}` : ''}
        </span>
      </div>

      {selectedItemType === 'log' && selectedLog ? (
        <OverlayScrollbar className="flex-1 pb-4" direction="vertical">
          <table className="w-full text-[11px] border-collapse text-[#cccccc]">
            <tbody>
              <tr className="border-b border-[#2b2b2b]">
                <td className="p-2 font-bold text-gray-400 bg-white/5 w-20">Time</td>
                <td className="p-2 font-mono text-gray-300">{selectedLog.timestamp}</td>
              </tr>
              <tr className="border-b border-[#2b2b2b]">
                <td className="p-2 font-bold text-gray-400 bg-white/5">Level</td>
                <td className="p-2">
                  <span className={`${getLevelColor(selectedLog.level)} font-bold uppercase`}>{selectedLog.level}</span>
                </td>
              </tr>
              <tr className="border-b border-[#2b2b2b]">
                <td className="p-2 font-bold text-gray-400 bg-white/5">Type</td>
                <td className="p-2">
                  <span className={`${getTypeColor(selectedLog.log_type)} font-semibold`}>
                    {LOG_TYPE_LABELS[selectedLog.log_type] ?? selectedLog.log_type.toUpperCase()}
                  </span>
                </td>
              </tr>
              {selectedLog.source && (
                <tr className="border-b border-[#2b2b2b]">
                  <td className="p-2 font-bold text-gray-400 bg-white/5">Source</td>
                  <td className="p-2 text-cyan-400 font-mono">{selectedLog.source}</td>
                </tr>
              )}
              <tr className="border-b border-[#2b2b2b]">
                <td className="p-2 font-bold text-gray-400 bg-white/5 align-top">Message</td>
                <td className="p-2">
                  <pre className="text-[11px] font-mono text-gray-200 whitespace-pre-wrap break-all leading-relaxed">{selectedLog.message}</pre>
                </td>
              </tr>
            </tbody>
          </table>
        </OverlayScrollbar>
      ) : selectedData ? (
        <OverlayScrollbar className="flex-1 pb-4" direction="vertical">
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
                        value={dataTypeKind(selectedData.dataType)}
                        options={[
                          { label: 'Boolean', value: 'Boolean' },
                          { label: 'Int32', value: 'Int32' },
                          { label: 'Int64', value: 'Int64' },
                          { label: 'Float32', value: 'Float32' },
                          { label: 'Float64', value: 'Float64' },
                          { label: 'String', value: 'String' },
                          { label: 'Object', value: 'Object' },
                          { label: 'Any', value: 'Any' },
                          { label: 'DataFrame', value: 'DataFrame' },
                          { label: 'Array', value: 'Array' },
                        ]}
                        onChange={(val) => handleUpdate({ dataType: dataTypeFromKey(val as string) })}
                      />
                    </td>
                  </tr>
                  {selectedData.dataType.kind !== "Array" && isPrimitiveType(selectedData.dataType) && (
                    <tr className="border-b border-[#2b2b2b]">
                      <td className="p-2 font-bold text-gray-400 bg-white/5">Value</td>
                      <td className="p-2">
                        {(selectedData.dataType.kind === "Boolean") ? (
                          <input
                            type="checkbox"
                            className="rounded text-[var(--accent-color)] focus:ring-[var(--accent-color)] bg-transparent border-[#2b2b2b]"
                            checked={!!dataValueToRaw(selectedData.dataValue)}
                            onChange={(e) => handleUpdate({ dataValue: dataValueFromRaw(e.target.checked, selectedData.dataType) })}
                          />
                        ) : (
                          <input
                            className="w-full bg-transparent border-none focus:ring-0 p-0 font-medium"
                            type={selectedData.dataType.kind === "String" ? "text" : "number"}
                            value={String(dataValueToRaw(selectedData.dataValue) ?? '')}
                            onChange={(e) => {
                              const val = selectedData.dataType.kind === "String"
                                ? e.target.value
                                : Number(e.target.value);
                              handleUpdate({ dataValue: dataValueFromRaw(val, selectedData.dataType) });
                            }}
                          />
                        )}
                      </td>
                    </tr>
                  )}
                </>
              )}
              {selectedItemType === 'function' && (
                <tr className="border-b border-[#2b2b2b]">
                  <td className="p-2 font-bold text-gray-400 bg-white/5">Type</td>
                  <td className="p-2 text-gray-400 italic">
                    {selectedItemType.charAt(0).toUpperCase() + selectedItemType.slice(1)}
                  </td>
                </tr>
              )}
              {selectedItemType === 'data' && (
                <>
                  <tr className="border-b border-[#2b2b2b]">
                    <td className="p-2 font-bold text-gray-400 bg-white/5">Columns</td>
                    <td className="p-2 text-gray-400">
                      {selectedData.columnCount || selectedData.columns?.length || 0} columns
                    </td>
                  </tr>
                  {selectedData.columns && selectedData.columns.length > 0 && (
                    <tr className="border-b border-[#2b2b2b]">
                      <td colSpan={2} className="p-0">
                        <OverlayScrollbar className="max-h-40 bg-black/20" direction="vertical">
                          <table className="w-full text-[9px]">
                            <thead>
                              <tr className="text-gray-500 border-b border-[#2b2b2b]">
                                <th className="p-1 text-left font-normal uppercase">Column</th>
                                <th className="p-1 text-left font-normal uppercase">Type</th>
                              </tr>
                            </thead>
                            <tbody>
                              {selectedData.columns.map((col: any) => (
                                <tr key={col.name} className="border-b border-white/5 hover:bg-white/5 transition-colors">
                                  <td className="p-1 text-gray-300 font-medium">{col.name}</td>
                                  <td className="p-1 text-[var(--accent-color)]/70">{col.type}</td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </OverlayScrollbar>
                      </td>
                    </tr>
                  )}
                  <tr className="border-b border-[#2b2b2b]">
                    <td className="p-2 font-bold text-gray-400 bg-white/5">Rows</td>
                    <td className="p-2 text-gray-400">
                      {selectedData.rowCount || selectedData.rows?.length || 0} rows
                    </td>
                  </tr>
                  {selectedData.sourcePath && (
                    <tr className="border-b border-[#2b2b2b]">
                      <td className="p-2 font-bold text-gray-400 bg-white/5">Source</td>
                      <td className="p-2 text-gray-400 break-all text-[9px]">
                        {selectedData.sourcePath}
                      </td>
                    </tr>
                  )}
                </>
              )}

            </tbody>
          </table>

          {selectedItemType === 'function' && (
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
        </OverlayScrollbar>
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
});

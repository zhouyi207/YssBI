import { forwardRef, useMemo } from "react";
import { useEditorGroup } from "@/features/application/editor";
import { Select } from "@/shared/ui";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
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
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => {
              const newPins = [...pins, { id: `pin-${crypto.randomUUID()}`, name: "NewPin", type: "int" }];
              handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
            }}
            className="text-muted-foreground hover:text-[var(--accent-color)]"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </Button>
        </div>
        <div className="space-y-1">
          {pins.map((pin, idx) => (
            <div key={pin.id} className="flex gap-1 items-center bg-white/5 p-1 rounded group">
              <Input
                className="h-6 flex-1 border-0 bg-transparent px-1 py-0 text-[10px] shadow-none"
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
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={() => {
                  const newPins = [...pins];
                  const current = newPins[idx].containerType;
                  const next = current === 'array' ? 'dataseries' : current === 'dataseries' ? undefined : 'array';
                  newPins[idx] = { ...newPins[idx], containerType: next };
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
                className={pin.containerType ? 'bg-blue-500/10 text-blue-400' : 'text-muted-foreground'}
                title={`Container: ${pin.containerType ?? 'none'} (click to cycle)`}
              >
                <span className="text-[9px] font-black">{pin.containerType === 'dataseries' ? '◇' : pin.containerType === 'array' ? '[]' : '·'}</span>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={() => {
                  const newPins = pins.filter((_, i) => i !== idx);
                  handleUpdate(isInput ? { inputs: newPins } : { outputs: newPins });
                }}
                className="opacity-0 transition-opacity group-hover:opacity-100 hover:text-red-500"
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </Button>
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
          <Table className="text-[11px] text-[#cccccc]">
            <TableBody>
              <TableRow>
                <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Time</TableCell>
                <TableCell className="font-mono text-gray-300">{selectedLog.timestamp}</TableCell>
              </TableRow>
              <TableRow>
                <TableCell className="bg-white/5 font-bold text-gray-400">Level</TableCell>
                <TableCell>
                  <span className={`${getLevelColor(selectedLog.level)} font-bold uppercase`}>{selectedLog.level}</span>
                </TableCell>
              </TableRow>
              <TableRow>
                <TableCell className="bg-white/5 font-bold text-gray-400">Type</TableCell>
                <TableCell>
                  <span className={`${getTypeColor(selectedLog.log_type)} font-semibold`}>
                    {LOG_TYPE_LABELS[selectedLog.log_type] ?? selectedLog.log_type.toUpperCase()}
                  </span>
                </TableCell>
              </TableRow>
              {selectedLog.source && (
                <TableRow>
                  <TableCell className="bg-white/5 font-bold text-gray-400">Source</TableCell>
                  <TableCell className="font-mono text-cyan-400">{selectedLog.source}</TableCell>
                </TableRow>
              )}
              <TableRow>
                <TableCell className="bg-white/5 align-top font-bold text-gray-400">Message</TableCell>
                <TableCell>
                  <pre className="text-[11px] font-mono text-gray-200 whitespace-pre-wrap break-all leading-relaxed">{selectedLog.message}</pre>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </OverlayScrollbar>
      ) : selectedData ? (
        <OverlayScrollbar className="flex-1 pb-4" direction="vertical">
          <Table className="text-[11px] text-[#cccccc]">
            <TableBody>
              <TableRow>
                <TableCell className="w-20 bg-white/5 font-bold text-gray-400">Name</TableCell>
                <TableCell>
                  <Input
                    className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                    value={selectedData.name}
                    onChange={(e) => handleUpdate({ name: e.target.value })}
                  />
                </TableCell>
              </TableRow>
              {selectedItemType === 'variable' && (
                <>
                  <TableRow>
                    <TableCell className="bg-white/5 font-bold text-gray-400">Type</TableCell>
                    <TableCell>
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
                    </TableCell>
                  </TableRow>
                  {selectedData.dataType.kind !== "Array" && isPrimitiveType(selectedData.dataType) && (
                    <TableRow>
                      <TableCell className="bg-white/5 font-bold text-gray-400">Value</TableCell>
                      <TableCell>
                        {(selectedData.dataType.kind === "Boolean") ? (
                          <Input
                            type="checkbox"
                            className="h-4 w-4 accent-[var(--accent-color)]"
                            checked={!!dataValueToRaw(selectedData.dataValue)}
                            onChange={(e) => handleUpdate({ dataValue: dataValueFromRaw(e.target.checked, selectedData.dataType) })}
                          />
                        ) : (
                          <Input
                            className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
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
                      </TableCell>
                    </TableRow>
                  )}
                </>
              )}
              {selectedItemType === 'function' && (
                <TableRow>
                  <TableCell className="bg-white/5 font-bold text-gray-400">Type</TableCell>
                  <TableCell className="text-gray-400 italic">
                    {selectedItemType.charAt(0).toUpperCase() + selectedItemType.slice(1)}
                  </TableCell>
                </TableRow>
              )}
              {selectedItemType === 'data' && (
                <>
                  <TableRow>
                    <TableCell className="bg-white/5 font-bold text-gray-400">Columns</TableCell>
                    <TableCell className="text-gray-400">
                      {selectedData.columnCount || selectedData.columns?.length || 0} columns
                    </TableCell>
                  </TableRow>
                  {selectedData.columns && selectedData.columns.length > 0 && (
                    <TableRow>
                      <TableCell colSpan={2} className="p-0">
                        <OverlayScrollbar className="max-h-40 bg-black/20" direction="vertical">
                          <Table className="text-[9px]">
                            <TableHeader>
                              <TableRow className="text-gray-500">
                                <TableHead className="h-6 p-1 font-normal uppercase">Column</TableHead>
                                <TableHead className="h-6 p-1 font-normal uppercase">Type</TableHead>
                              </TableRow>
                            </TableHeader>
                            <TableBody>
                              {selectedData.columns.map((col: any) => (
                                <TableRow key={col.name} className="border-white/5">
                                  <TableCell className="p-1 font-medium text-gray-300">{col.name}</TableCell>
                                  <TableCell className="p-1 text-[var(--accent-color)]/70">{col.type}</TableCell>
                                </TableRow>
                              ))}
                            </TableBody>
                          </Table>
                        </OverlayScrollbar>
                      </TableCell>
                    </TableRow>
                  )}
                  <TableRow>
                    <TableCell className="bg-white/5 font-bold text-gray-400">Rows</TableCell>
                    <TableCell className="text-gray-400">
                      {selectedData.rowCount || selectedData.rows?.length || 0} rows
                    </TableCell>
                  </TableRow>
                  {selectedData.sourcePath && (
                    <TableRow>
                      <TableCell className="bg-white/5 font-bold text-gray-400">Source</TableCell>
                      <TableCell className="break-all text-[9px] text-gray-400">
                        {selectedData.sourcePath}
                      </TableCell>
                    </TableRow>
                  )}
                </>
              )}

            </TableBody>
          </Table>

          {selectedItemType === 'function' && (
            <>
              {renderPinEditor("Inputs", selectedData.inputs, true)}
              {renderPinEditor("Outputs", selectedData.outputs, false)}
            </>
          )}
          <div className="p-2">
            <Button
              type="button"
              variant="destructive"
              size="sm"
              onClick={handleDelete}
              className="mt-4 w-full uppercase tracking-wider"
            >
              Delete {selectedItemType}
            </Button>
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

import { forwardRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorGroup } from '@/features/application/editor';
import { useEditorStore } from '@/features/core/editor';
import { useLogStore } from '@/features/core/log/logStore';
import { DetailEmptyState } from './DetailEmptyState';
import { VariableDetailPanel } from './panels/VariableDetailPanel';
import { EventDetailPanel } from './panels/EventDetailPanel';
import { FunctionDetailPanel } from './panels/FunctionDetailPanel';
import { DataDetailPanel } from './panels/DataDetailPanel';
import { LogDetailPanel } from './panels/LogDetailPanel';
import { NodeDetailPanel } from './panels/NodeDetailPanel';

export const Detail = forwardRef<HTMLDivElement, { width?: number }>((_, ref) => {
  const { t } = useTranslation();
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
    deleteDataFrame,
  } = useEditorGroup();

  const selectedGraphId = useEditorStore((s) => s.selectedGraphId);
  const selectedLog = useLogStore((s) => s.selectedLog);

  const selectedData = useMemo(() => {
    if (!selectedItemId || !selectedItemType) return null;
    if (selectedItemType === 'variable') return Variables[selectedItemId];
    if (selectedItemType === 'event') return events[selectedItemId];
    if (selectedItemType === 'function') return functions[selectedItemId];
    if (selectedItemType === 'data') return dataframes[selectedItemId];
    return null;
  }, [selectedItemId, selectedItemType, Variables, events, functions, dataframes]);

  const clearSelection = () => setSelectedInfo(null, null);

  return (
    <div
      ref={ref}
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
      onWheel={(e) => e.stopPropagation()}
    >
      {selectedItemType === 'log' && selectedLog ? (
        <LogDetailPanel log={selectedLog} />
      ) : selectedItemType === 'node' && selectedItemId && selectedGraphId ? (
        <NodeDetailPanel nodeId={selectedItemId} graphId={selectedGraphId} />
      ) : selectedData && selectedItemType === 'variable' ? (
        <VariableDetailPanel
          variable={selectedData}
          onUpdate={(patch) => updateVariable(selectedItemId!, patch)}
          onDelete={() => deleteVariable(selectedItemId!)}
          onDeleted={clearSelection}
        />
      ) : selectedData && selectedItemType === 'event' ? (
        <EventDetailPanel
          event={selectedData}
          onUpdate={(patch) => updateEvent(selectedItemId!, patch)}
          onDelete={() => deleteEvent(selectedItemId!)}
          onDeleted={clearSelection}
        />
      ) : selectedData && selectedItemType === 'function' ? (
        <FunctionDetailPanel
          fn={selectedData}
          onUpdate={(patch) => updateFunction(selectedItemId!, patch)}
          onDelete={() => deleteFunction(selectedItemId!)}
          onDeleted={clearSelection}
        />
      ) : selectedData && selectedItemType === 'data' ? (
        <DataDetailPanel
          dataframe={selectedData}
          onUpdate={(patch) => updateDataFrame(selectedItemId!, patch)}
          onDelete={() => deleteDataFrame(selectedItemId!)}
        />
      ) : (
        <>
          <div
            className="flex shrink-0 items-center justify-between border-b border-border bg-[var(--workbench-bg)]/50 px-3"
            style={{ height: 'var(--titlebar-height)' }}
          >
            <span className="text-[10px] font-black uppercase tracking-widest text-gray-500">
              {t('detail.title')}
            </span>
          </div>
          <DetailEmptyState />
        </>
      )}
    </div>
  );
});

Detail.displayName = 'Detail';

import { forwardRef, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorGroup } from '@/features/application/editor';
import { useLogStore } from '@/features/core/log/logStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { renameResource } from '@/features/application/resource/resourceActions';
import { updateFunctionSignature } from '@/features/application/graphDocument/graphDocumentActions';
import { DetailEmptyState } from './DetailEmptyState';
import { VariableDetailPanel } from './panels/VariableDetailPanel';
import { EventDetailPanel } from './panels/EventDetailPanel';
import { FunctionDetailPanel } from './panels/FunctionDetailPanel';
import { DataDetailPanel } from './panels/DataDetailPanel';
import { LogDetailPanel } from './panels/LogDetailPanel';
import { NodeDetailPanel } from './panels/NodeDetailPanel';
import { WorksheetDetailPanel } from './panels/WorksheetDetailPanel';
import { detailSectionTitleClass } from './shared/detailStyles';
import { workbenchPanelHeaderClass } from '../workbenchPanelHeaderStyles';

export const Detail = forwardRef<HTMLDivElement, { width?: number }>((_, ref) => {
  const { t } = useTranslation();
  const {
    Variables,
    events,
    functions,
    dataframes,
    selectedItemId,
    selectedItemType,
    updateVariable,
    updateDataFrame,
  } = useEditorGroup();

  const selectedLog = useLogStore((s) => s.selectedLog);
  const worksheetDocument = useWorksheetStore((s) =>
    selectedItemId && selectedItemType === 'worksheet'
      ? s.documents[selectedItemId] ?? null
      : null,
  );
  const selectedFunctionSignature = useGraphMetaStore((s) =>
    selectedItemId && selectedItemType === 'function'
      ? s.graphs[selectedItemId]
      : undefined,
  );

  useEffect(() => {
    if (selectedItemType !== 'worksheet' || !selectedItemId) return;
    if (worksheetDocument) return;
    void WorksheetService.loadWorksheet(selectedItemId)
      .then((loaded) => useWorksheetStore.getState().upsertDocument(loaded))
      .catch(() => undefined);
  }, [selectedItemId, selectedItemType, worksheetDocument]);

  const selectedData = useMemo(() => {
    if (!selectedItemId || !selectedItemType) return null;
    if (selectedItemType === 'variable') return Variables[selectedItemId];
    if (selectedItemType === 'event') return events[selectedItemId];
    if (selectedItemType === 'function') {
      const fn = functions[selectedItemId];
      if (!fn) return null;
      return {
        ...fn,
        inputs: selectedFunctionSignature?.functionInputs ?? [],
        outputs: selectedFunctionSignature?.functionOutputs ?? [],
      };
    }
    if (selectedItemType === 'data') return dataframes[selectedItemId];
    return null;
  }, [selectedItemId, selectedItemType, Variables, events, functions, dataframes, selectedFunctionSignature]);

  return (
    <div
      ref={ref}
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
      onWheel={(e) => e.stopPropagation()}
    >
      {selectedItemType === 'log' && selectedLog ? (
        <LogDetailPanel log={selectedLog} />
      ) : selectedItemType === 'node' && selectedItemId ? (
        <NodeDetailPanel nodeId={selectedItemId} />
      ) : selectedData && selectedItemType === 'variable' ? (
        <VariableDetailPanel
          variable={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: selectedItemId!, kind: 'variable' }, patch.name);
              return;
            }
            updateVariable(selectedItemId!, patch);
          }}
        />
      ) : selectedData && selectedItemType === 'event' ? (
        <EventDetailPanel
          event={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: selectedItemId!, kind: 'event' }, patch.name);
            }
          }}
        />
      ) : selectedData && selectedItemType === 'function' ? (
        <FunctionDetailPanel
          fn={selectedData}
          onRename={(name) => {
            void renameResource({ id: selectedItemId!, kind: 'function' }, name);
          }}
          onSignatureChange={(patch) => {
            void updateFunctionSignature(selectedItemId!, patch);
          }}
        />
      ) : selectedItemType === 'worksheet' && worksheetDocument ? (
        <WorksheetDetailPanel document={worksheetDocument} />
      ) : selectedData && selectedItemType === 'data' ? (
        <DataDetailPanel
          dataframe={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: selectedItemId!, kind: 'database' }, patch.name);
              return;
            }
            updateDataFrame(selectedItemId!, patch);
          }}
        />
      ) : (
        <div className="flex h-full min-h-0 flex-col bg-background/40">
          <div className={workbenchPanelHeaderClass}>
            <span className={detailSectionTitleClass}>
              {t('detail.title')}
            </span>
          </div>
          <DetailEmptyState />
        </div>
      )}
    </div>
  );
});

Detail.displayName = 'Detail';

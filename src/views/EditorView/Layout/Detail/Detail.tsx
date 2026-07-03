import { forwardRef, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorGroup } from '@/features/application/editor';
import { useDetailTarget } from '@/features/core/editor';
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
    updateVariable,
    updateDataFrame,
  } = useEditorGroup();

  const target = useDetailTarget();
  const selectedLog = useLogStore((s) => s.selectedLog);

  const targetId = target && 'id' in target ? target.id : null;

  const worksheetDocument = useWorksheetStore((s) =>
    target?.kind === 'worksheet' && targetId ? s.documents[targetId] ?? null : null,
  );
  const selectedFunctionSignature = useGraphMetaStore((s) =>
    target?.kind === 'function' && targetId ? s.graphs[targetId] : undefined,
  );

  useEffect(() => {
    if (target?.kind !== 'worksheet' || !targetId) return;
    if (worksheetDocument) return;
    void WorksheetService.loadWorksheet(targetId)
      .then((loaded) => useWorksheetStore.getState().upsertDocument(loaded))
      .catch(() => undefined);
  }, [target?.kind, targetId, worksheetDocument]);

  const selectedData = useMemo(() => {
    if (!target || !targetId) return null;
    if (target.kind === 'variable') return Variables[targetId];
    if (target.kind === 'event') return events[targetId];
    if (target.kind === 'function') {
      const fn = functions[targetId];
      if (!fn) return null;
      return {
        ...fn,
        inputs: selectedFunctionSignature?.functionInputs ?? [],
        outputs: selectedFunctionSignature?.functionOutputs ?? [],
      };
    }
    if (target.kind === 'data') return dataframes[targetId];
    return null;
  }, [target, targetId, Variables, events, functions, dataframes, selectedFunctionSignature]);

  return (
    <div
      ref={ref}
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
      onWheel={(e) => e.stopPropagation()}
    >
      {target?.kind === 'log' && selectedLog ? (
        <LogDetailPanel log={selectedLog} />
      ) : target?.kind === 'node' ? (
        <NodeDetailPanel nodeId={target.id} />
      ) : selectedData && target?.kind === 'variable' ? (
        <VariableDetailPanel
          variable={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: targetId!, kind: 'variable' }, patch.name);
              return;
            }
            updateVariable(targetId!, patch);
          }}
        />
      ) : selectedData && target?.kind === 'event' ? (
        <EventDetailPanel
          event={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: targetId!, kind: 'event' }, patch.name);
            }
          }}
        />
      ) : selectedData && target?.kind === 'function' ? (
        <FunctionDetailPanel
          fn={selectedData}
          onRename={(name) => {
            void renameResource({ id: targetId!, kind: 'function' }, name);
          }}
          onSignatureChange={(patch) => {
            void updateFunctionSignature(targetId!, patch);
          }}
        />
      ) : target?.kind === 'worksheet' && worksheetDocument ? (
        <WorksheetDetailPanel document={worksheetDocument} />
      ) : selectedData && target?.kind === 'data' ? (
        <DataDetailPanel
          dataframe={selectedData}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: targetId!, kind: 'database' }, patch.name);
              return;
            }
            updateDataFrame(targetId!, patch);
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

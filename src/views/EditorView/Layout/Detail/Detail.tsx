import { forwardRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorGroup } from '@/features/application/editor';
import { useDetailTarget } from '@/features/core/editor';
import { useLogStore } from '@/features/core/log/logStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { renameGraph } from '@/features/application/dataManagement/graphActions';
import { renameResource } from '@/features/application/resource/resourceActions';
import { updateFunctionSignature } from '@/features/application/graphDocument/graphDocumentActions';
import { uiStore } from '@/features/core/ui/UIStore';
import { DetailEmptyState } from './DetailEmptyState';
import { VariableDetailPanel } from './panels/VariableDetailPanel';
import { EventDetailPanel } from './panels/EventDetailPanel';
import { FunctionDetailPanel } from './panels/FunctionDetailPanel';
import { DataDetailPanel } from './panels/DataDetailPanel';
import { LogDetailPanel } from './panels/LogDetailPanel';
import { NodeDetailPanel } from './panels/NodeDetailPanel';
import { NodeDefinitionDetailPanel } from './panels/NodeDefinitionDetailPanel';
import { WorksheetDetailPanel } from './panels/WorksheetDetailPanel';
import { detailSectionTitleClass } from './shared/detailStyles';
import { workbenchPanelHeaderClass } from '../workbenchPanelHeaderStyles';

export const Detail = forwardRef<HTMLDivElement, { width?: number }>((_, ref) => {
  const { t } = useTranslation();
  const {
    variables,
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

  const selectedFunction = target?.kind === 'function' && targetId ? functions[targetId] : undefined;

  return (
    <div
      ref={ref}
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
    >
      {target?.kind === 'log' && selectedLog ? (
        <LogDetailPanel log={selectedLog} />
      ) : target?.kind === 'node' ? (
        <NodeDetailPanel nodeId={target.id} />
      ) : target?.kind === 'nodeDefinition' ? (
        <NodeDefinitionDetailPanel nodeType={target.nodeType} />
      ) : target?.kind === 'variable' && targetId && variables[targetId] ? (
        <VariableDetailPanel
          variable={variables[targetId]}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameResource({ id: targetId!, kind: 'variable' }, patch.name);
              return;
            }
            updateVariable(targetId!, patch);
          }}
        />
      ) : target?.kind === 'event' && targetId && events[targetId] ? (
        <EventDetailPanel
          event={events[targetId]}
          onUpdate={(patch) => {
            if (typeof patch.name === 'string') {
              void renameGraph(targetId!, patch.name, 'event');
            }
          }}
        />
      ) : target?.kind === 'function' && targetId && selectedFunction ? (
        <FunctionDetailPanel
          fn={{
            ...selectedFunction,
            inputs: selectedFunctionSignature?.functionInputs ?? [],
            outputs: selectedFunctionSignature?.functionOutputs ?? [],
          }}
          onRename={(name) => {
            void renameGraph(targetId!, name, 'function');
          }}
          onSignatureChange={(patch) => {
            void updateFunctionSignature(targetId!, patch).then(({ sideEffectWarning }) => {
              if (sideEffectWarning) {
                uiStore.showToast(t('detail.signature.sideEffectWarning'), 'warning');
              }
            });
          }}
        />
      ) : target?.kind === 'worksheet' && worksheetDocument ? (
        <WorksheetDetailPanel document={worksheetDocument} />
      ) : target?.kind === 'data' && targetId && dataframes[targetId] ? (
        <DataDetailPanel
          dataframe={{
            id: targetId,
            name: typeof dataframes[targetId].name === 'string' ? dataframes[targetId].name : targetId,
            columnCount: typeof dataframes[targetId].columnCount === 'number' ? dataframes[targetId].columnCount : undefined,
            columns: Array.isArray(dataframes[targetId].columns)
              ? (dataframes[targetId].columns as Array<{ name: string; type: string }>)
              : undefined,
            rowCount: typeof dataframes[targetId].rowCount === 'number' ? dataframes[targetId].rowCount : undefined,
            rows: Array.isArray(dataframes[targetId].rows) ? (dataframes[targetId].rows as unknown[]) : undefined,
            sourcePath: typeof dataframes[targetId].sourcePath === 'string' ? dataframes[targetId].sourcePath : undefined,
          }}
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

import { forwardRef, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorSessionDetailActions } from '@/features/application/editor';
import { createVariableAction } from '@/features/application/dataManagement/variableActions';
import { focusDetail } from '@/features/core/editor/detail/detailFocusCommands';
import { selectLocalVariableEntriesForGraph } from '@/features/core/variable/variableScopeSelectors';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
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
import { useFunctionCallSites } from '@/features/application/graphDocument/useFunctionCallSites';
import { useEditorSessionResources } from '@/features/application/editor';
import { useDetailPanelModel } from './useDetailPanelModel';

export const Detail = forwardRef<HTMLDivElement>((_, ref) => {
  const { t } = useTranslation();
  const { updateVariable, updateDataFrame } = useEditorSessionDetailActions();
  const { variables } = useEditorSessionResources();
  const { model, worksheetTargetId, worksheetDocument } = useDetailPanelModel();
  const functionPath = model.kind === 'function' ? model.path : undefined;
  const functionLocalVariables = useMemo(
    () =>
      functionPath
        ? selectLocalVariableEntriesForGraph(variables, functionPath, 'function')
        : [],
    [variables, functionPath],
  );
  const { sites: functionCallSites, loading: functionCallSitesLoading } =
    useFunctionCallSites(functionPath);

  useEffect(() => {
    if (!worksheetTargetId || worksheetDocument) return;
    void WorksheetService.loadWorksheet(worksheetTargetId)
      .then((loaded) => useWorksheetStore.getState().upsertDocument(loaded))
      .catch(() => undefined);
  }, [worksheetTargetId, worksheetDocument]);

  const content = (() => {
    switch (model.kind) {
      case 'log':
        return <LogDetailPanel log={model.log} />;
      case 'node':
        return <NodeDetailPanel nodeId={model.nodeId} />;
      case 'nodeDefinition':
        return <NodeDefinitionDetailPanel nodeType={model.nodeType} />;
      case 'variable':
        return (
          <VariableDetailPanel
            variable={model.variable}
            onUpdate={(patch) => {
              if (typeof patch.name === 'string') {
                void renameResource({ id: model.id, kind: 'variable' }, patch.name);
                return;
              }
              updateVariable(model.id, patch);
            }}
          />
        );
      case 'event':
        return (
          <EventDetailPanel
            event={model.event}
            onUpdate={(patch) => {
              if (typeof patch.name === 'string') {
                void renameGraph(model.path, patch.name, 'event');
              }
            }}
          />
        );
      case 'function':
        return (
          <FunctionDetailPanel
            fn={model.fn}
            callSites={functionCallSites}
            callSitesLoading={functionCallSitesLoading}
            localVariables={functionLocalVariables}
            onSelectLocalVariable={(id) => focusDetail({ kind: 'variable', id })}
            onAddLocalVariable={() => {
              void createVariableAction({
                activeGraphPath: model.path,
                graphType: 'function',
                isGlobal: false,
              });
            }}
            onRename={(name) => {
              void renameGraph(model.path, name, 'function');
            }}
            onSignatureChange={(patch) => {
              void updateFunctionSignature(model.path, patch).then(({ sideEffectWarning }) => {
                if (sideEffectWarning) {
                  uiStore.showToast(t('detail.signature.sideEffectWarning'), 'warning');
                }
              });
            }}
          />
        );
      case 'worksheet':
        return <WorksheetDetailPanel document={model.document} />;
      case 'data':
        return (
          <DataDetailPanel
            dataframe={model.dataframe}
            onUpdate={(patch) => {
              if (typeof patch.name === 'string') {
                void renameResource({ id: model.id, kind: 'database' }, patch.name);
                return;
              }
              updateDataFrame(model.id, patch);
            }}
          />
        );
      case 'empty':
        return (
          <div className="flex h-full min-h-0 flex-col bg-background/40">
            <div className={workbenchPanelHeaderClass}>
              <span className={detailSectionTitleClass}>{t('detail.title')}</span>
            </div>
            <DetailEmptyState />
          </div>
        );
      default: {
        const exhaustive: never = model;
        return exhaustive;
      }
    }
  })();

  return (
    <div
      ref={ref}
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
    >
      {content}
    </div>
  );
});

Detail.displayName = 'Detail';

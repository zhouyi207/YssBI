import { useEffect } from 'react';
import { useEditorSessionDetailActions } from '@/features/application/editor';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { updateFunctionSignature } from '@/features/application/graphDocument/graphDocumentActions';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { DetailEmptyState } from './DetailEmptyState';
import { DataDetailPanel } from './panels/DataDetailPanel';
import { EventDetailPanel } from './panels/EventDetailPanel';
import { FunctionDetailPanel } from './panels/FunctionDetailPanel';
import { LogDetailPanel } from './panels/LogDetailPanel';
import { NodeDefinitionDetailPanel } from './panels/NodeDefinitionDetailPanel';
import { NodeDetailPanel } from './panels/NodeDetailPanel';
import { VariableDetailPanel } from './panels/VariableDetailPanel';
import { WorksheetDetailPanel } from './panels/WorksheetDetailPanel';
import { useDetailPanelModel } from './useDetailPanelModel';

export function DetailsPane() {
  const { updateVariable } = useEditorSessionDetailActions();
  const { model, worksheetPath, worksheetName, worksheetDocument } = useDetailPanelModel();

  useEffect(() => {
    if (!worksheetPath || worksheetDocument) return;
    const context = captureProjectCommandContext();
    void WorksheetService.loadWorksheet(context.projectInstanceId, worksheetPath)
      .then((loaded) => {
        if (context.isCurrent()) {
          useWorksheetStore.getState().upsertDocument(worksheetPath, loaded);
        }
      })
      .catch(() => undefined);
  }, [worksheetPath, worksheetDocument]);

  switch (model.kind) {
    case 'log':
      return <LogDetailPanel log={model.log} />;
    case 'node':
      return <NodeDetailPanel graphPath={model.graphPath} nodeId={model.nodeId} />;
    case 'nodeDefinition':
      return <NodeDefinitionDetailPanel nodeType={model.nodeType} />;
    case 'variable':
      return (
        <VariableDetailPanel
          variable={model.variable}
          onUpdate={(patch) => updateVariable(model.id, patch)}
        />
      );
    case 'event':
      return <EventDetailPanel event={model.event} />;
    case 'function':
      return (
        <FunctionDetailPanel
          fn={model.fn}
          onSignatureChange={(patch) => {
            void updateFunctionSignature(model.path, patch);
          }}
        />
      );
    case 'worksheet':
      return (
        <WorksheetDetailPanel
          worksheetPath={worksheetPath!}
          name={worksheetName ?? ''}
          document={model.document}
        />
      );
    case 'data':
      return <DataDetailPanel dataframe={model.dataframe} />;
    case 'empty':
      return (
        <div className="flex h-full min-h-0 flex-col bg-background">
          <DetailEmptyState />
        </div>
      );
    default: {
      const exhaustive: never = model;
      return exhaustive;
    }
  }
}

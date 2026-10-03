import { updateFunctionSignature } from "@/features/application/graphDocument/graphDocumentActions";
import { DetailEmptyState } from "./DetailEmptyState";
import { DataDetailPanel } from "./panels/DataDetailPanel";
import { EventDetailPanel } from "./panels/EventDetailPanel";
import { FunctionDetailPanel } from "./panels/FunctionDetailPanel";
import { LogDetailPanel } from "./panels/LogDetailPanel";
import { NodeDefinitionDetailPanel } from "./panels/NodeDefinitionDetailPanel";
import { NodeDetailPanel } from "./panels/NodeDetailPanel";
import { ChartDetailPanel } from "./panels/ChartDetailPanel";
import { MindDetailPanel } from "./panels/MindDetailPanel";
import { FileDetailPanel } from "./panels/FileDetailPanel";
import { useDetailPanelModel } from "./useDetailPanelModel";

export function DetailsPane() {
  const model = useDetailPanelModel();

  switch (model.kind) {
    case "log":
      return <LogDetailPanel log={model.log} />;
    case "node":
      return <NodeDetailPanel graphPath={model.graphPath} nodeId={model.nodeId} />;
    case "nodeDefinition":
      return <NodeDefinitionDetailPanel nodeType={model.nodeType} />;
    case "event_graph":
      return <EventDetailPanel event={model.event} graphPath={model.path} />;
    case "function_graph":
      return (
        <FunctionDetailPanel
          graphPath={model.path}
          fn={model.fn}
          onSignatureChange={(patch) => {
            void updateFunctionSignature(model.path, patch);
          }}
        />
      );
    case "chart":
      return (
        <ChartDetailPanel
          key={model.path}
          chartPath={model.path}
          name={model.name}
          document={model.document}
        />
      );
    case "data":
      return <DataDetailPanel key={model.dataframe.id} dataframe={model.dataframe} />;
    case "mind":
      return (
        <MindDetailPanel
          key={`${model.path}:${model.panelInstanceId}`}
          path={model.path}
          panelInstanceId={model.panelInstanceId}
          nodeId={model.nodeId}
        />
      );
    case "doc":
      return <FileDetailPanel resourceKind="doc" resourceRef={model.path} />;
    case "unavailable":
      return (
        <FileDetailPanel
          resourceKind={model.resourceKind}
          resourceRef={model.resourceRef}
          status="unavailable"
        />
      );
    case "empty":
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

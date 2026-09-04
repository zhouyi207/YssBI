import { AssistantPanel } from "@/modules/assistant/public";
import { commandsActivityPanelContribution } from "@/modules/commands/public";
import { dataActivityPanelContribution } from "@/modules/data-explorer/public";
import { DetailsPane, InspectPane } from "@/modules/details/public";
import { nodeCatalogActivityPanelContribution } from "@/modules/node-catalog/public";
import { projectActivityPanelContribution } from "@/modules/project-explorer/public";
import { ResultPanel } from "@/modules/results/public";
import { GraphProblemsPanel } from "@/modules/problems/public";
import { RunOutputPanel } from "@/modules/output/public";
import {
  EditorResourceDockPanel,
  type RootDockviewPanelComponent,
  type RootPanelRegistry,
} from "@/modules/workbench/public";
import { LogDomainDockviewHost } from "@/modules/logs/public";
import { editorRendererRegistry } from "./editorRendererRegistry";

const EditorResourcePanel: RootDockviewPanelComponent = (props) => {
  return <EditorResourceDockPanel {...props} rendererRegistry={editorRendererRegistry} />;
};

function MainLogsDockPanel() {
  return <LogDomainDockviewHost layout={{ kind: "main" }} />;
}

const ResultDockPanel: RootDockviewPanelComponent = ({ params }) => {
  const { metadata } = params;
  return metadata.role === "result" ? <ResultPanel resultId={metadata.resultId} /> : null;
};

export const rootPanelRegistry = {
  EditorResource: EditorResourcePanel,
  Project: projectActivityPanelContribution,
  Nodes: nodeCatalogActivityPanelContribution,
  Data: dataActivityPanelContribution,
  Commands: commandsActivityPanelContribution,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  Inspect: InspectPane,
  Result: ResultDockPanel,
  Logs: MainLogsDockPanel,
  Output: RunOutputPanel,
  Problems: GraphProblemsPanel,
} satisfies RootPanelRegistry;

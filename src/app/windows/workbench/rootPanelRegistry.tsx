import { AssistantPanel } from "@/modules/assistant/public";
import { commandsActivityPanelContribution } from "@/modules/commands/public";
import { dataActivityPanelContribution } from "@/modules/data-explorer/public";
import { DetailsPane, InspectPane } from "@/modules/details/public";
import { nodeCatalogActivityPanelContribution } from "@/modules/node-catalog/public";
import { projectActivityPanelContribution } from "@/modules/project-explorer/public";
import { ResultPanel } from "@/modules/results/public";
import {
  EditorResourceDockPanel,
  type RootDockviewPanelComponent,
  type RootPanelRegistry,
} from "@/modules/workbench/public";
import { DiagnosticsPanel, LogDomainDockviewHost, OutputPanel } from "@/modules/logs/public";
import { editorRendererRegistry } from "./editorRendererRegistry";

const EditorResourcePanel: RootDockviewPanelComponent = (props) => {
  return <EditorResourceDockPanel {...props} rendererRegistry={editorRendererRegistry} />;
};

function MainLogsDockPanel() {
  return <LogDomainDockviewHost layout={{ kind: "main" }} />;
}

export const rootPanelRegistry = {
  EditorResource: EditorResourcePanel,
  Project: projectActivityPanelContribution,
  Nodes: nodeCatalogActivityPanelContribution,
  Data: dataActivityPanelContribution,
  Commands: commandsActivityPanelContribution,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  Inspect: InspectPane,
  Result: ResultPanel,
  Logs: MainLogsDockPanel,
  Output: OutputPanel,
  Diagnostics: DiagnosticsPanel,
} satisfies RootPanelRegistry;

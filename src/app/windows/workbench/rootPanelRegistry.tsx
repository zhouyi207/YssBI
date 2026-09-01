import { AssistantPanel } from "@/views/AssistantView/AssistantPanel";
import { EditorResourceDockPanel } from "@/views/EditorView/Layout/EditorResourceDockPanel";
import { DetailsPane } from "@/views/EditorView/Layout/Detail/DetailsPane";
import { InspectPane } from "@/views/EditorView/Layout/Detail/InspectPane";
import { ResultPanel } from "@/views/EditorView/Layout/result/ResultPanel";
import { commandsActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/commands/public";
import { dataActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/data/public";
import { nodeCatalogActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/nodes/public";
import { projectActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/project/public";
import type {
  RootDockviewPanelComponent,
  RootPanelRegistry,
} from "@/views/EditorView/Layout/RootDockviewHost";
import { DiagnosticsPanel } from "@/views/LogView/DiagnosticsPanel";
import { LogDomainDockviewHost } from "@/views/LogView/LogDomainDockviewHost";
import { OutputPanel } from "@/views/LogView/OutputPanel";
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

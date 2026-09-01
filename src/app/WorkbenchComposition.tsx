import { AssistantPanel } from "@/views/AssistantView/AssistantPanel";
import { EditorResourceDockPanel } from "@/views/EditorView/Layout/EditorResourceDockPanel";
import { DetailsPane } from "@/views/EditorView/Layout/Detail/DetailsPane";
import { InspectPane } from "@/views/EditorView/Layout/Detail/InspectPane";
import { ResultPanel } from "@/views/EditorView/Layout/result/ResultPanel";
import {
  WorkbenchCommandsPanel,
  WorkbenchDataPanel,
  WorkbenchNodesPanel,
  WorkbenchProjectPanel,
} from "@/views/EditorView/Layout/WorkbenchActivityPanels";
import { createRootPanelRegistry } from "@/views/EditorView/Layout/RootDockviewHost";
import { WorkbenchWindow } from "@/views/EditorView/WorkbenchWindow";
import { DiagnosticsPanel } from "@/views/LogView/DiagnosticsPanel";
import { LogWorkspaceDockview } from "@/views/LogView/LogWorkspaceDockview";
import { OutputPanel } from "@/views/LogView/OutputPanel";

function MainLogsDockPanel() {
  return <LogWorkspaceDockview layout={{ kind: "main" }} />;
}

const rootPanelRegistry = createRootPanelRegistry({
  EditorResource: EditorResourceDockPanel,
  Project: WorkbenchProjectPanel,
  Nodes: WorkbenchNodesPanel,
  Data: WorkbenchDataPanel,
  Commands: WorkbenchCommandsPanel,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  Inspect: InspectPane,
  Result: ResultPanel,
  Logs: MainLogsDockPanel,
  Output: OutputPanel,
  Diagnostics: DiagnosticsPanel,
});

export function WorkbenchComposition() {
  return <WorkbenchWindow panelRegistry={rootPanelRegistry} />;
}

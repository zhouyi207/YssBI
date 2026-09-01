import { useCallback } from "react";

import { AssistantPanel } from "@/views/AssistantView/AssistantPanel";
import { EditorResourceDockPanel } from "@/views/EditorView/Layout/EditorResourceDockPanel";
import { DetailsPane } from "@/views/EditorView/Layout/Detail/DetailsPane";
import { InspectPane } from "@/views/EditorView/Layout/Detail/InspectPane";
import { ResultPanel } from "@/views/EditorView/Layout/result/ResultPanel";
import { commandsActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/commands/public";
import { dataActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/data/public";
import { nodeCatalogActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/nodes/public";
import { projectActivityPanelContribution } from "@/views/EditorView/Layout/activityPanels/project/public";
import { createRootPanelRegistry } from "@/views/EditorView/Layout/RootDockviewHost";
import { WorkbenchWindow } from "@/views/EditorView/WorkbenchWindow";
import { WatermarkView } from "@/views/EditorView/Canvas/overlays/WatermarkView";
import { DiagnosticsPanel } from "@/views/LogView/DiagnosticsPanel";
import { LogDomainDockviewHost } from "@/views/LogView/LogDomainDockviewHost";
import { OutputPanel } from "@/views/LogView/OutputPanel";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";

function MainLogsDockPanel() {
  return <LogDomainDockviewHost layout={{ kind: "main" }} />;
}

const rootPanelRegistry = createRootPanelRegistry({
  EditorResource: EditorResourceDockPanel,
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
});

export function WorkbenchComposition() {
  const dndCoordinator = useActivityEditorDndCoordinator();
  const commands = useWorkbenchCommandCoordinator();
  const watermarkComponent = useCallback(() => <WatermarkView commands={commands} />, [commands]);

  return (
    <WorkbenchWindow
      panelRegistry={rootPanelRegistry}
      dndCoordinator={dndCoordinator}
      commands={commands}
      watermarkComponent={watermarkComponent}
    />
  );
}

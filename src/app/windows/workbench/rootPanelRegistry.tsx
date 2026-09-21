import { SettingsView, SettingsWindowHeader } from "@/modules/settings/public";
import { requestCloseWorkbenchPanel } from "@/features/application/editor/workbenchPanelClose";
import { AssistantPanel } from "@/modules/assistant/public";
import { PluginsPanel, PluginViewFrame } from "@/modules/plugins/public";
import { usePlugins } from "./integrations/PluginProvider";
import { useEffect, useState } from "react";
import { commandsActivityPanelContribution } from "@/modules/commands/public";
import { DetailsPane } from "@/modules/details/public";
import { nodeCatalogActivityPanelContribution } from "@/modules/node-catalog/public";
import { projectActivityPanelContribution } from "@/modules/project-explorer/public";
import { ResultPanel } from "@/modules/results/public";
import { GraphProblemsPanel } from "@/modules/problems/public";
import { RunFailurePanel } from "@/modules/output/public";
import {
  EditorResourcePanel,
  type RootPanelComponent,
  type RootPanelRegistry,
} from "@/modules/workbench/public";
import { LogDomainLayoutHost } from "@/modules/logs/public";
import { editorRendererRegistry } from "./editorRendererRegistry";

const RegisteredEditorPanel: RootPanelComponent = (props) => {
  return <EditorResourcePanel {...props} rendererRegistry={editorRendererRegistry} />;
};

function MainLogsDockPanel() {
  return <LogDomainLayoutHost layout={{ kind: "main" }} />;
}

function PluginsDockPanel() {
  const runtime = usePlugins();
  return (
    <PluginsPanel
      plugins={runtime.plugins}
      loading={runtime.loading}
      busy={runtime.busy}
      error={runtime.error}
      onRefresh={() => void runtime.refresh()}
      onInstall={() => void runtime.install()}
      onOpen={(plugin) => {
        const view = plugin.manifest.contributes.views[0];
        if (view) runtime.open(plugin.manifest.id, view);
      }}
      onToggle={(plugin) => void runtime.setEnabled(plugin)}
      onUninstall={(plugin) => void runtime.uninstall(plugin)}
    />
  );
}

const PluginDockPanel: RootPanelComponent = ({ params, visible }) => {
  const runtime = usePlugins();
  const [activated, setActivated] = useState(visible);
  useEffect(() => {
    if (visible) setActivated(true);
  }, [visible]);
  const metadata = params.metadata;
  if (metadata.role !== "plugin" || !activated) return null;
  return (
    <PluginViewFrame
      plugin={runtime.plugins.find((plugin) => plugin.manifest.id === metadata.pluginId)}
      viewId={metadata.viewId}
      visible={visible}
      onOpen={runtime.open}
    />
  );
};

const ResultDockPanel: RootPanelComponent = ({ params }) => {
  const { metadata } = params;
  return metadata.role === "result" ? <ResultPanel reference={metadata.reference} /> : null;
};

export const floatingPanelHeader: RootPanelComponent = ({ panelInstanceId }) => (
  <SettingsWindowHeader onRequestClose={() => void requestCloseWorkbenchPanel(panelInstanceId)} />
);

export const rootPanelRegistry = {
  EditorResource: RegisteredEditorPanel,
  Project: projectActivityPanelContribution,
  Nodes: nodeCatalogActivityPanelContribution,
  Commands: commandsActivityPanelContribution,
  Plugins: PluginsDockPanel,
  Plugin: PluginDockPanel,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  Settings: SettingsView,
  Result: ResultDockPanel,
  Logs: MainLogsDockPanel,
  Output: RunFailurePanel,
  Problems: GraphProblemsPanel,
} satisfies RootPanelRegistry;

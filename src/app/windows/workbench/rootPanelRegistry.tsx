import { AssistantPanel } from "@/modules/assistant/public";
import { PluginsPanel, PluginViewFrame } from "@/modules/plugins/public";
import { usePlugins } from "./integrations/PluginProvider";
import { useEffect, useState } from "react";
import { commandsActivityPanelContribution } from "@/modules/commands/public";
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

const PluginDockPanel: RootDockviewPanelComponent = ({ params, api }) => {
  const runtime = usePlugins();
  const [activated, setActivated] = useState(api.isVisible);
  const [visible, setVisible] = useState(api.isVisible);
  useEffect(() => {
    const disposable = api.onDidVisibilityChange((event) => {
      setVisible(event.isVisible);
      if (event.isVisible) setActivated(true);
    });
    if (api.isVisible) setActivated(true);
    setVisible(api.isVisible);
    return () => disposable.dispose();
  }, [api]);
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

const ResultDockPanel: RootDockviewPanelComponent = ({ params }) => {
  const { metadata } = params;
  return metadata.role === "result" ? (
    <ResultPanel resultId={metadata.resultId} source={metadata.source} />
  ) : null;
};

export const rootPanelRegistry = {
  EditorResource: EditorResourcePanel,
  Project: projectActivityPanelContribution,
  Nodes: nodeCatalogActivityPanelContribution,
  Commands: commandsActivityPanelContribution,
  Plugins: PluginsDockPanel,
  Plugin: PluginDockPanel,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  Inspect: InspectPane,
  Result: ResultDockPanel,
  Logs: MainLogsDockPanel,
  Output: RunOutputPanel,
  Problems: GraphProblemsPanel,
} satisfies RootPanelRegistry;

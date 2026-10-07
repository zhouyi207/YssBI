import { AssistantPanel, AssistantConversationPanel } from "@/modules/assistant/public";
import { PluginsPanel, PluginViewFrame } from "@/modules/plugins/public";
import { usePluginActions, usePlugins } from "./integrations/PluginProvider";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { commandsActivityPanelContribution } from "@/modules/commands/public";
import { DetailsPane } from "@/modules/details/public";
import { nodeCatalogActivityPanelContribution } from "@/modules/node-catalog/public";
import { projectActivityPanelContribution } from "@/modules/project-explorer/public";
import { ResultPanel } from "@/modules/results/public";
import { ReferencePanel } from "@/modules/document-editor/public";
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
  const { t } = useTranslation();
  const { plugins, loading, busy, error } = usePlugins(
    useShallow((state) => ({
      plugins: state.byId,
      loading: state.loading,
      busy: state.busy,
      error: state.error,
    })),
  );
  const actions = usePluginActions();
  return (
    <PluginsPanel
      plugins={plugins}
      loading={loading}
      busy={busy}
      error={error ? t(error) : null}
      onRefresh={() => void actions.refresh()}
      onInstall={() => void actions.install(t)}
      onOpen={(plugin) => {
        const view = plugin.manifest.contributes.views[0];
        if (view) actions.open(plugin.manifest.id, view);
      }}
      onToggle={(plugin) => void actions.setEnabled(plugin)}
      onUninstall={(plugin) => void actions.uninstall(plugin, t)}
    />
  );
}

const PluginDockPanel: RootPanelComponent = ({ params, visible }) => {
  const metadata = params.metadata;
  const plugin = usePlugins((state) =>
    metadata.role === "plugin" ? state.byId.get(metadata.pluginId) : undefined,
  );
  const actions = usePluginActions();
  const [activated, setActivated] = useState(visible);
  useEffect(() => {
    if (visible) setActivated(true);
  }, [visible]);
  if (metadata.role !== "plugin" || !activated) return null;
  return (
    <PluginViewFrame
      plugin={plugin}
      viewId={metadata.viewId}
      visible={visible}
      onOpen={actions.open}
    />
  );
};

const ResultDockPanel: RootPanelComponent = ({ params }) => {
  const { metadata } = params;
  return metadata.role === "result" ? <ResultPanel reference={metadata.reference} /> : null;
};

const ReferenceDockPanel: RootPanelComponent = ({ params }) => {
  const { metadata } = params;
  return metadata.role === "reference" ? (
    <ReferencePanel url={metadata.url} title={metadata.title} />
  ) : null;
};

export const rootPanelRegistry = {
  EditorResource: RegisteredEditorPanel,
  Project: projectActivityPanelContribution,
  Nodes: nodeCatalogActivityPanelContribution,
  Commands: commandsActivityPanelContribution,
  Plugins: PluginsDockPanel,
  Plugin: PluginDockPanel,
  Details: DetailsPane,
  Assistant: AssistantPanel,
  AssistantConversation: ({ params }) =>
    params.metadata.role === "conversation" ? (
      <AssistantConversationPanel sessionId={params.metadata.sessionId} />
    ) : null,
  Result: ResultDockPanel,
  Reference: ReferenceDockPanel,
  Logs: MainLogsDockPanel,
  Output: RunFailurePanel,
  Problems: GraphProblemsPanel,
} satisfies RootPanelRegistry;

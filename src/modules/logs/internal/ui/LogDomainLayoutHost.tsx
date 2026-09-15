import { useEffect, useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { Actions, Layout } from "flexlayout-react";
import { logsLayoutRootBinding } from "@/modules/workbench/public";
import { useSettingsRead } from "@/features/core/settings/read";
import { isLogDomainId } from "@/features/application/log";
import { resolveYssbiLayoutTheme } from "@/shared/theme/layoutTheme";
import { LogDomainPanel } from "./LogDomainPanel";
import { LogWorkspaceActions } from "./LogWorkspaceActions";
import { LogWorkspaceProvider } from "./logWorkspaceContext";
import { LOG_DOMAIN_TITLE_KEYS } from "./logPresentation";

export type LogDomainLayoutLifecycle = { readonly kind: "main" } | { readonly kind: "ephemeral" };
export interface LogDomainLayoutHostProps {
  readonly layout: LogDomainLayoutLifecycle;
}
export function LogDomainLayoutHost({ layout }: LogDomainLayoutHostProps) {
  const { t } = useTranslation();
  const [binding] = useState(logsLayoutRootBinding.create);
  const model = useSyncExternalStore(binding.subscribe, binding.getModel, binding.getModel);
  const themeMode = useSettingsRead((state) => state.theme.mode);
  const isMainLayout = layout.kind === "main";
  useEffect(() => {
    if (!isMainLayout) return;
    const token = logsLayoutRootBinding.bind(binding);
    return () => logsLayoutRootBinding.unbind(token);
  }, [binding, isMainLayout]);
  return (
    <LogWorkspaceProvider presentation={isMainLayout ? "embedded" : "standalone"}>
      <div
        data-yssbi-logs-layout
        className={"workbench-logs-layout " + resolveYssbiLayoutTheme(themeMode)}
      >
        <Layout
          model={model}
          supportsPopout={false}
          invalidateTabContentOnParentRender={false}
          factory={(node) => {
            const domain = node.getConfig()?.domain;
            return isLogDomainId(domain) ? <LogDomainPanel domain={domain} /> : null;
          }}
          onAction={(action) =>
            [
              Actions.DELETE_TAB,
              Actions.DELETE_TABSET,
              Actions.POPOUT_TAB,
              Actions.POPOUT_TABSET,
            ].includes(action.type)
              ? undefined
              : action
          }
          onRenderTab={(node, values) => {
            const domain = node.getConfig()?.domain;
            if (isLogDomainId(domain)) values.content = t(LOG_DOMAIN_TITLE_KEYS[domain]);
          }}
          onRenderTabSet={(node, values) => {
            const domain = node.getSelectedNode()?.getConfig()?.domain;
            values.buttons.push(
              <LogWorkspaceActions
                key="log-actions"
                domain={isLogDomainId(domain) ? domain : undefined}
              />,
            );
          }}
        />
      </div>
    </LogWorkspaceProvider>
  );
}

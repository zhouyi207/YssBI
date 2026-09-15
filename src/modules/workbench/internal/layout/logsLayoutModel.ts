import type { IJsonModel } from "flexlayout-react";
import {
  isLogDomainId,
  LOG_DOMAIN_ORDER,
  logDomainPanelId,
  logDomainTitle,
  type LogDomainId,
} from "@/features/domain/log/logDomains";
import { isLayoutJson, isRecord } from "./layoutSerialization";

export const LOGS_LAYOUT_COMPONENT_ID = "LogDomainPanel" as const;
export const LOGS_LAYOUT_DEFAULT_GROUP_ID = "logs-domain-group";
export interface LogsLayoutPanelParams {
  readonly domain: LogDomainId;
}

export function createDefaultLogsLayout(): IJsonModel {
  return {
    global: {
      tabEnableClose: false,
      tabEnableFloat: false,
      tabEnablePopout: false,
      tabEnableRename: false,
      tabEnablePin: false,
      tabSetEnableDrag: false,
      tabSetEnableDivide: false,
      tabSetEnableClose: false,
      tabSetEnableMaximize: false,
      tabSetEnableActiveIcon: false,
    },
    layout: {
      type: "row",
      children: [
        {
          type: "tabset",
          id: LOGS_LAYOUT_DEFAULT_GROUP_ID,
          active: true,
          selected: 0,
          children: LOG_DOMAIN_ORDER.map((domain) => ({
            type: "tab",
            id: logDomainPanelId(domain),
            component: LOGS_LAYOUT_COMPONENT_ID,
            name: logDomainTitle(domain),
            config: { domain },
          })),
        },
      ],
    },
  };
}
export const DEFAULT_LOGS_LAYOUT = createDefaultLogsLayout();
export function isValidLogsLayout(candidate: unknown): candidate is IJsonModel {
  const domains = new Set<LogDomainId>();
  const valid = isLayoutJson(candidate, (tab, parent) => {
    const config = tab.config;
    if (
      parent !== LOGS_LAYOUT_DEFAULT_GROUP_ID ||
      tab.component !== LOGS_LAYOUT_COMPONENT_ID ||
      !isRecord(config) ||
      !isLogDomainId(config.domain) ||
      domains.has(config.domain) ||
      tab.id !== logDomainPanelId(config.domain)
    )
      return false;
    domains.add(config.domain);
    return true;
  });
  return valid && domains.size === LOG_DOMAIN_ORDER.length;
}

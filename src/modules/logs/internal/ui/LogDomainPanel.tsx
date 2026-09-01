import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { IDockviewPanelProps } from "dockview-react";

import type { LogsDockviewPanelParams } from "@/modules/workbench/public";
import { isLogDomainId, applyLogFilter } from "@/features/application/log";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";
import { LogPanelList } from "./LogPanelList";
import { LOG_DOMAIN_TITLE_KEYS } from "./logPresentation";
import { useLogWorkspaceContext } from "./logWorkspaceContext";

function isSameDiagnostic(left: DiagnosticRecordDto, right: DiagnosticRecordDto): boolean {
  return left.streamId === right.streamId && left.sequence === right.sequence;
}

export function LogDomainPanel(props: IDockviewPanelProps<LogsDockviewPanelParams>) {
  const domain = props.params?.domain;
  if (!isLogDomainId(domain)) {
    throw new Error("LogDomainPanel requires a valid domain parameter");
  }

  const { t } = useTranslation();
  const localizedTitle = t(LOG_DOMAIN_TITLE_KEYS[domain]);
  const currentTitle = props.api.title;
  useEffect(() => {
    if (currentTitle !== localizedTitle) props.api.setTitle(localizedTitle);
  }, [currentTitle, localizedTitle, props.api]);

  const {
    logs,
    filter,
    selectedLog,
    autoScroll,
    isInitialLoad,
    refreshScrollToken,
    presentation,
    selectLog,
  } = useLogWorkspaceContext();
  const filteredLogs = useMemo(() => applyLogFilter(logs, filter, domain), [domain, filter, logs]);
  const selectedIndex = useMemo(() => {
    if (!selectedLog) return null;
    const index = filteredLogs.findIndex((log) => isSameDiagnostic(log, selectedLog));
    return index >= 0 ? index : null;
  }, [filteredLogs, selectedLog]);

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <LogPanelList
        filteredLogs={filteredLogs}
        totalLogCount={logs.length}
        isInitialLoad={isInitialLoad}
        autoScroll={autoScroll}
        refreshScrollToken={refreshScrollToken}
        presentation={presentation}
        selectedIndex={selectedIndex}
        onSelectLog={selectLog}
      />
    </div>
  );
}

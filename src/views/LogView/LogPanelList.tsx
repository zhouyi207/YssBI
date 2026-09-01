import { useTranslation } from "react-i18next";
import { VscFile } from "react-icons/vsc";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";
import { LogPanelVirtualList } from "./LogPanelVirtualList";
import type { LogPanelPresentation } from "./useLogPanelVirtualList";

export interface LogPanelListProps {
  readonly filteredLogs: readonly DiagnosticRecordDto[];
  readonly totalLogCount: number;
  readonly isInitialLoad: boolean;
  readonly autoScroll: boolean;
  readonly refreshScrollToken: number;
  readonly presentation: LogPanelPresentation;
  readonly selectedIndex: number | null;
  readonly onSelectLog: (log: DiagnosticRecordDto) => void;
}

export function LogPanelList({
  filteredLogs,
  totalLogCount,
  isInitialLoad,
  autoScroll,
  refreshScrollToken,
  presentation,
  selectedIndex,
  onSelectLog,
}: LogPanelListProps) {
  const { t } = useTranslation();

  if (isInitialLoad) {
    return (
      <div className="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-background text-muted-foreground">
        <div className="size-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
        <p className="text-xs">{t("log.loadingLogs")}</p>
      </div>
    );
  }

  if (filteredLogs.length === 0) {
    return (
      <Empty className="relative min-h-0 rounded-none bg-background px-6">
        <EmptyHeader>
          <EmptyMedia variant="icon" className="text-muted-foreground">
            <VscFile />
          </EmptyMedia>
          <EmptyTitle>{totalLogCount === 0 ? t("log.noLogs") : t("log.noMatches")}</EmptyTitle>
          <EmptyDescription>
            {totalLogCount === 0 ? t("log.runGraphHint") : t("log.adjustFilterHint")}
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <LogPanelVirtualList
      filteredLogs={filteredLogs}
      autoScroll={autoScroll}
      refreshScrollToken={refreshScrollToken}
      presentation={presentation}
      selectedIndex={selectedIndex}
      onSelectLog={onSelectLog}
    />
  );
}

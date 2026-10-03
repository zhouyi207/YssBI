import { useTranslation } from "react-i18next";
import { VscFile } from "react-icons/vsc";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { LogPanelVirtualList } from "./LogPanelVirtualList";

export interface LogPanelListProps {
  readonly filteredLogs: readonly LogRecordDto[];
  readonly hasLogs: boolean;
  readonly isInitialLoad: boolean;
  readonly autoScroll: boolean;
  readonly refreshScrollToken: number;
  readonly selectedIndex: number | null;
  readonly onSelectLog: (log: LogRecordDto) => void;
}

export function LogPanelList({
  filteredLogs,
  hasLogs,
  isInitialLoad,
  autoScroll,
  refreshScrollToken,
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
          <EmptyTitle>{hasLogs ? t("log.noMatches") : t("log.noLogs")}</EmptyTitle>
          <EmptyDescription>
            {hasLogs ? t("log.adjustFilterHint") : t("log.runGraphHint")}
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
      selectedIndex={selectedIndex}
      onSelectLog={onSelectLog}
    />
  );
}

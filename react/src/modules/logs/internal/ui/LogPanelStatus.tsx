import { useTranslation } from "react-i18next";
import { useFilteredLogs, useLiveLogs, type LogDomainId } from "@/features/application/log";
import { useLogWorkspaceContext } from "./logWorkspaceContext";

const STATUS_COLOR = {
  connecting: "bg-amber-400 animate-pulse",
  live: "bg-emerald-500/80",
  error: "bg-red-400",
} as const;

export interface LogPanelStatusProps {
  readonly domain: LogDomainId;
}

export function LogPanelStatus({ domain }: LogPanelStatusProps) {
  const { t } = useTranslation();
  const { subscriptionStatus } = useLogWorkspaceContext();
  const filteredLogCount = useFilteredLogs(domain, (entries) => entries.length);
  const totalLogCount = useLiveLogs((snapshot) => snapshot.entries.length);
  const truncated = useLiveLogs((snapshot) => snapshot.truncated);
  const showTruncated = truncated && subscriptionStatus !== "error";

  return (
    <div className="flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
      <span
        className={`size-1.5 shrink-0 rounded-full ${showTruncated ? "bg-amber-400" : STATUS_COLOR[subscriptionStatus]}`}
        aria-hidden
      />
      <span className="truncate">
        {t("log.showCount", { filtered: filteredLogCount, total: totalLogCount })}
      </span>
      {showTruncated ? (
        <span
          role="status"
          className="truncate text-amber-600 dark:text-amber-400"
          title={t("log.streamTruncated")}
        >
          {t("log.streamTruncated")}
        </span>
      ) : subscriptionStatus === "error" ? (
        <span role="status" className="text-destructive">
          {t("log.streamDisconnected")}
        </span>
      ) : null}
    </div>
  );
}

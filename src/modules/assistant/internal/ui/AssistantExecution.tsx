import { useEffect, useState } from "react";
import { useAuiState } from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";

export function AssistantElapsed({
  startedAt,
  completedAt,
  updatedAt,
  running,
}: {
  startedAt: number;
  completedAt?: number | null;
  updatedAt?: number;
  running: boolean;
}) {
  const { t, i18n } = useTranslation();
  const connected = useAssistantHarnessSnapshot(
    (state) => state.status !== "error" && state.status !== "initializing" && state.isRunning,
  );
  const active = running && completedAt == null && connected;
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [active, startedAt]);
  if (completedAt == null && !active)
    return (
      <span className="text-[11px] text-muted-foreground">
        {t("panel.assistantTimingUnconfirmed")}
      </span>
    );
  const seconds = Math.max(0, ((completedAt ?? now) - startedAt) / 1000);
  const clock = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "medium" });
  const title = [
    t("panel.assistantStartedAt", { value: clock.format(startedAt) }),
    completedAt != null
      ? t("panel.assistantFinishedAt", { value: clock.format(completedAt) })
      : updatedAt != null
        ? t("panel.assistantUpdatedAt", { value: clock.format(updatedAt) })
        : null,
  ]
    .filter(Boolean)
    .join("\n");
  const value =
    seconds < 60
      ? t("panel.assistantDurationSeconds", {
          value: new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 }).format(seconds),
        })
      : seconds < 3600
        ? t("panel.assistantDurationMinutes", {
            minutes: Math.floor(seconds / 60),
            seconds: Math.floor(seconds % 60),
          })
        : t("panel.assistantDurationHours", {
            hours: Math.floor(seconds / 3600),
            minutes: Math.floor((seconds % 3600) / 60),
            seconds: Math.floor(seconds % 60),
          });
  return (
    <span
      title={title}
      className="shrink-0 text-[11px] font-normal tabular-nums text-muted-foreground"
    >
      {t(completedAt == null ? "panel.assistantElapsed" : "panel.assistantDuration", { value })}
    </span>
  );
}

export function AssistantTurnTiming() {
  const startedAt = useAuiState((state) => state.message.createdAt.getTime());
  const finishedAt = useAuiState((state) => state.message.metadata.custom.finishedAt);
  const running = useAuiState((state) => state.message.status?.type === "running");
  const updatedAt = useAuiState((state) => state.message.metadata.custom.updatedAt);
  return (
    <AssistantElapsed
      startedAt={startedAt}
      completedAt={typeof finishedAt === "number" ? finishedAt : null}
      updatedAt={typeof updatedAt === "number" ? updatedAt : undefined}
      running={running}
    />
  );
}

export function AssistantTurnModel() {
  const model = useAuiState((state) => state.message.metadata.custom.model);
  if (!model || typeof model !== "object" || !("modelName" in model) || !("providerName" in model))
    return null;
  if (typeof model.modelName !== "string" || typeof model.providerName !== "string") return null;
  return (
    <span
      className="max-w-52 truncate text-[11px] font-normal text-muted-foreground"
      title={`${model.providerName} · ${model.modelName}`}
    >
      {model.modelName}
    </span>
  );
}

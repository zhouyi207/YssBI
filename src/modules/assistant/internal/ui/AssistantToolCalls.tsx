import { useEffect, useState, type PropsWithChildren } from "react";
import { useAuiState, type ToolCallMessagePartComponent } from "@assistant-ui/react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import {
  VscCheck,
  VscChevronRight,
  VscCopy,
  VscError,
  VscLoading,
  VscTools,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";
import { HarnessService } from "@/services/assistant/harnessService";
import type { HarnessToolInspection } from "@/services/assistant/harnessContract";
import type { ToolCallMessagePart } from "@assistant-ui/react";
import { assistantFailureKey } from "@/features/application/assistant/assistantMessageContent";
import { AssistantElapsed } from "./AssistantExecution";
import { AssistantArtifacts } from "./AssistantResources";
import {
  assistantLinkResource,
  openAssistantResource,
} from "@/features/application/assistant/assistantResourceActions";

function toolState(result: unknown, isError?: boolean) {
  const status =
    typeof result === "object" && result !== null && "status" in result ? result.status : undefined;
  if (typeof result === "object" && result !== null && "executionStatus" in result) {
    if (result.executionStatus === "succeeded") return "assistantGraphSucceeded";
    if (result.executionStatus === "failed") return "assistantGraphFailed";
  }
  switch (status) {
    case "cancelled":
      return "assistantToolCancelled";
    case "timed-out":
      return "assistantToolTimedOut";
    case "interrupted":
      return "assistantToolInterrupted";
    case "unknown":
      return "assistantToolUnknown";
    case "failed":
      return "assistantToolFailed";
    default:
      return isError
        ? "assistantToolFailed"
        : result === undefined
          ? "assistantToolRunning"
          : "assistantToolCompleted";
  }
}

export function AssistantToolGroup({
  startIndex,
  endIndex,
  children,
}: PropsWithChildren<{ startIndex: number; endIndex: number }>) {
  const { t } = useTranslation();
  const { count, running, failed, activeToolName } = useAuiState(
    useShallow((state) => {
      let count = 0;
      let running = 0;
      let failed = 0;
      let activeToolName: string | null = null;
      for (let index = startIndex; index <= endIndex; index += 1) {
        const part = state.message.content[index];
        if (part?.type !== "tool-call") continue;
        count += 1;
        const status = toolState(part.result, part.isError);
        if (status === "assistantToolRunning") {
          running += 1;
          activeToolName ??= part.toolName;
        } else if (status !== "assistantToolCompleted" && status !== "assistantGraphSucceeded") {
          failed += 1;
        }
      }
      return { count, running, failed, activeToolName };
    }),
  );
  const [expanded, setExpanded] = useState<boolean | undefined>(undefined);
  const connected = useAssistantHarnessSnapshot(
    (state) => state.status !== "error" && state.status !== "initializing" && state.isRunning,
  );
  const active = running > 0 && connected;
  return (
    <Collapsible
      open={expanded ?? active}
      onOpenChange={setExpanded}
      className="my-2 min-w-0 rounded-lg border border-border/60 bg-muted/20 text-xs"
    >
      <CollapsibleTrigger className="group flex w-full min-w-0 flex-wrap items-center gap-2 rounded-lg px-3 py-2.5 text-left text-muted-foreground hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring">
        <VscChevronRight
          aria-hidden
          className="shrink-0 transition-transform group-data-[state=open]:rotate-90 motion-reduce:transition-none"
        />
        {active ? (
          <VscLoading aria-hidden className="shrink-0 motion-safe:animate-spin" />
        ) : (
          <VscTools aria-hidden className="shrink-0" />
        )}
        <span className="shrink-0">
          {t(
            `panel.${active ? "assistantToolsRunning" : running > 0 ? "assistantToolsUnconfirmed" : "assistantToolsFinished"}`,
            {
              count,
              completed: count - running,
            },
          )}
        </span>
        {failed > 0 && (
          <span className="shrink-0 text-destructive">
            {t("panel.assistantToolsFailed", { count: failed })}
          </span>
        )}
        {activeToolName !== null && (
          <span className="min-w-0 flex-1 truncate">
            {t(`panel.assistantToolNames.${activeToolName}`, { defaultValue: activeToolName })}
          </span>
        )}
      </CollapsibleTrigger>
      <CollapsibleContent className="min-w-0 border-t border-border/60 px-2 py-1">
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}

export function AssistantToolDetails({
  toolCallId,
  toolName,
  result,
  isError,
  timing,
}: Pick<ToolCallMessagePart, "toolCallId" | "toolName" | "result" | "isError" | "timing">) {
  const { t } = useTranslation();
  const sessionId = useAssistantHarnessSnapshot((state) => state.sessionId);
  const [expanded, setExpanded] = useState(false);
  const [inspection, setInspection] = useState<HarnessToolInspection | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [reload, setReload] = useState(0);
  const [openFailed, setOpenFailed] = useState(false);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const state = toolState(result, isError);
  const running = state === "assistantToolRunning";
  const connected = useAssistantHarnessSnapshot(
    (state) => state.status !== "error" && state.status !== "initializing" && state.isRunning,
  );
  const failed =
    !running && state !== "assistantToolCompleted" && state !== "assistantGraphSucceeded";
  useEffect(() => {
    if (!sessionId) return;
    let current = true;
    setLoadFailed(false);
    void HarnessService.inspectTool(sessionId, toolCallId).then(
      (value) => {
        if (current) setInspection(value);
      },
      () => {
        if (current) setLoadFailed(true);
      },
    );
    return () => {
      current = false;
    };
  }, [sessionId, toolCallId, running, reload]);
  const parameters = JSON.stringify(inspection?.parameters ?? {}, null, 2);
  const failureCode =
    result &&
    typeof result === "object" &&
    "failureCode" in result &&
    typeof result.failureCode === "string"
      ? result.failureCode
      : null;
  const recordedTiming = inspection
    ? { startedAt: inspection.startedAt, completedAt: inspection.finishedAt ?? timing?.completedAt }
    : timing;
  const targetResource = inspection?.target ? assistantLinkResource(inspection.target) : null;
  const label = t(`panel.assistantToolNames.${toolName}`, { defaultValue: toolName });
  const copyLabel = t(
    `panel.${copyState === "copied" ? "assistantCopied" : copyState === "failed" ? "assistantCopyFailed" : "assistantCopy"}`,
  );
  return (
    <Collapsible open={expanded} onOpenChange={setExpanded} className="min-w-0">
      <CollapsibleTrigger className="group flex w-full min-w-0 flex-wrap items-center gap-2 rounded px-2 py-2 text-left enabled:hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring">
        {running ? (
          <VscLoading
            aria-hidden
            className={`shrink-0 self-center text-muted-foreground ${connected ? "motion-safe:animate-spin" : ""}`}
          />
        ) : failed ? (
          <VscError aria-hidden className="shrink-0 self-center text-destructive" />
        ) : (
          <VscCheck aria-hidden className="shrink-0 self-center text-muted-foreground" />
        )}
        <span className="min-w-0 flex-1 truncate font-medium" title={toolName}>
          {label}
        </span>
        <span
          className={`shrink-0 text-[11px] ${failed ? "text-destructive" : "text-muted-foreground"}`}
        >
          {t(`panel.${running && !connected ? "assistantToolUnknown" : state}`)}
        </span>
        {recordedTiming && <AssistantElapsed {...recordedTiming} running={running} />}
        <VscChevronRight
          aria-hidden
          className="shrink-0 self-center text-muted-foreground group-data-[state=open]:rotate-90"
        />
        {inspection?.target && (
          <span
            className="w-full truncate pl-6 text-[11px] text-muted-foreground"
            title={inspection.target}
          >
            {inspection.target}
          </span>
        )}
      </CollapsibleTrigger>
      {failureCode && (
        <p className="px-2 pb-2 text-xs text-destructive">
          {t(`panel.assistantErrors.${assistantFailureKey(failureCode)}`, {
            defaultValue: failureCode,
          })}
        </p>
      )}
      <CollapsibleContent className="mx-2 mb-2 min-w-0 rounded border border-border/60 bg-background/50">
        <div className="min-w-0 p-2">
          <p className="mb-2 font-mono text-[11px] text-muted-foreground">{toolName}</p>
          {!inspection && !loadFailed && <p role="status">{t("panel.assistantLoadingDetails")}</p>}
          {loadFailed && (
            <div role="alert" className="mb-2 text-destructive">
              {t("panel.assistantDetailsFailed")}
              <Button
                type="button"
                variant="ghost"
                size="xs"
                onClick={() => setReload((value) => value + 1)}
              >
                {t("panel.assistantRetryDetails")}
              </Button>
            </div>
          )}
          {inspection && Object.entries(inspection.parameters).length > 0 && (
            <dl className="mb-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
              {Object.entries(inspection.parameters).map(([key, value]) => (
                <div key={key} className="contents">
                  <dt className="text-muted-foreground">
                    {t(`panel.assistantToolFacts.${key}`, { defaultValue: key })}
                  </dt>
                  <dd className="min-w-0 wrap-anywhere">
                    {typeof value === "string"
                      ? t(`panel.assistantToolValues.${value}`, { defaultValue: value })
                      : JSON.stringify(value)}
                  </dd>
                </div>
              ))}
            </dl>
          )}
          {inspection && (
            <AssistantArtifacts artifacts={inspection.artifacts} results={inspection.results} />
          )}
          {targetResource && (
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => {
                setOpenFailed(false);
                void openAssistantResource(targetResource).catch(() => setOpenFailed(true));
              }}
            >
              {t("panel.assistantOpenResource")} · {inspection?.target}
            </Button>
          )}
          {openFailed && (
            <p role="alert" className="text-destructive">
              {t("panel.assistantResourceOpenFailed")}
            </p>
          )}
          <details>
            <summary className="cursor-pointer text-muted-foreground">
              {t("panel.assistantTechnicalDetails")}
            </summary>
            <div className="flex items-start gap-2">
              <pre
                aria-label={t("panel.assistantToolArguments")}
                className="max-h-60 min-w-0 flex-1 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-5"
              >
                {parameters}
              </pre>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="shrink-0"
                aria-label={copyLabel}
                title={copyLabel}
                onBlur={() => setCopyState("idle")}
                onClick={async () => {
                  try {
                    await navigator.clipboard.writeText(parameters);
                    setCopyState("copied");
                  } catch {
                    setCopyState("failed");
                  }
                }}
              >
                {copyState === "copied" ? <VscCheck aria-hidden /> : <VscCopy aria-hidden />}
              </Button>
            </div>
            {copyState !== "idle" && (
              <span role="status" className="text-muted-foreground">
                {copyLabel}
              </span>
            )}
          </details>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

export const AssistantToolCall: ToolCallMessagePartComponent = AssistantToolDetails;

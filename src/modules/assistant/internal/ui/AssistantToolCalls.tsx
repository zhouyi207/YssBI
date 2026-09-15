import { useState, type PropsWithChildren } from "react";
import { useAuiState, type ToolCallMessagePartComponent } from "@assistant-ui/react";
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

function toolState(result: unknown, isError?: boolean) {
  const status =
    typeof result === "object" && result !== null && "status" in result ? result.status : undefined;
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
  const content = useAuiState((s) => s.message.content);
  const tools = content.slice(startIndex, endIndex + 1).filter((part) => part.type === "tool-call");
  const running = tools.filter(
    (part) => toolState(part.result, part.isError) === "assistantToolRunning",
  );
  const failed = tools.filter(
    (part) =>
      !["assistantToolRunning", "assistantToolCompleted"].includes(
        toolState(part.result, part.isError),
      ),
  );
  const [expanded, setExpanded] = useState<boolean | undefined>(undefined);
  const active = running[0];
  return (
    <Collapsible
      open={expanded ?? running.length > 0}
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
          {t(`panel.${active ? "assistantToolsRunning" : "assistantToolsFinished"}`, {
            count: tools.length,
            completed: tools.length - running.length,
          })}
        </span>
        {failed.length > 0 && (
          <span className="shrink-0 text-destructive">
            {t("panel.assistantToolsFailed", { count: failed.length })}
          </span>
        )}
        {active && (
          <span className="min-w-0 flex-1 truncate">
            {t(`panel.assistantToolNames.${active.toolName}`, { defaultValue: active.toolName })}
          </span>
        )}
      </CollapsibleTrigger>
      <CollapsibleContent className="min-w-0 border-t border-border/60 px-2 py-1">
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}

export const AssistantToolCall: ToolCallMessagePartComponent = ({
  toolName,
  args,
  argsText,
  result,
  isError,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const state = toolState(result, isError);
  const running = state === "assistantToolRunning";
  const failed = !running && state !== "assistantToolCompleted";
  const hasArgs = Object.keys(args).length > 0;
  const parameters = hasArgs
    ? JSON.stringify(args, null, 2)
    : argsText && argsText.trim() !== "{}"
      ? argsText
      : "";
  const preview = parameters.replace(/\s+/g, " ").slice(0, 160);
  const label = t(`panel.assistantToolNames.${toolName}`, { defaultValue: toolName });
  const copyLabel = t(
    `panel.${copyState === "copied" ? "assistantCopied" : copyState === "failed" ? "assistantCopyFailed" : "assistantCopy"}`,
  );
  return (
    <Collapsible open={expanded} onOpenChange={setExpanded} className="min-w-0">
      <CollapsibleTrigger className="group flex w-full min-w-0 items-center gap-2 rounded px-2 py-2 text-left hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring">
        {running ? (
          <VscLoading
            aria-hidden
            className="shrink-0 text-muted-foreground motion-safe:animate-spin"
          />
        ) : failed ? (
          <VscError aria-hidden className="shrink-0 text-destructive" />
        ) : (
          <VscCheck aria-hidden className="shrink-0 text-muted-foreground" />
        )}
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium" title={toolName}>
            {label}
          </span>
          {parameters && (
            <span className="block truncate font-mono text-[11px] text-muted-foreground">
              {preview}
              {parameters.replace(/\s+/g, " ").length > 160 ? "…" : ""}
            </span>
          )}
        </span>
        <span
          className={`shrink-0 text-[11px] ${failed ? "text-destructive" : "text-muted-foreground"}`}
        >
          {t(`panel.${state}`)}
        </span>
        <VscChevronRight
          aria-hidden
          className="shrink-0 text-muted-foreground group-data-[state=open]:rotate-90"
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="mx-2 mb-2 min-w-0 rounded border border-border/60 bg-background/50 p-2">
        <div className="break-all font-mono text-[11px] text-muted-foreground">{toolName}</div>
        <div className="mt-2 flex items-center justify-between gap-2 text-muted-foreground">
          <span>{t("panel.assistantToolArguments")}</span>
          {parameters && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
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
          )}
        </div>
        {copyState !== "idle" && (
          <span role="status" className="text-muted-foreground">
            {copyLabel}
          </span>
        )}
        {parameters ? (
          <pre className="mt-1 max-h-60 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-5">
            {parameters}
          </pre>
        ) : (
          <p className="mt-1 text-muted-foreground">
            {t("panel.assistantToolArgumentsUnavailable")}
          </p>
        )}
        {failed && <p className="mt-2 text-destructive">{t(`panel.${state}`)}</p>}
      </CollapsibleContent>
    </Collapsible>
  );
};

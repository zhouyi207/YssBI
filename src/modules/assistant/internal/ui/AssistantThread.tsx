import {
  AuiIf,
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  type DataMessagePartComponent,
  type ToolCallMessagePartComponent,
} from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { VscDebugStop, VscSend, VscSparkle, VscTrash } from "react-icons/vsc";

import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";

function UserMessage() {
  return (
    <MessagePrimitive.Root className="flex justify-end py-2">
      <div className="max-w-[88%] rounded-lg bg-primary px-3 py-2 text-xs/relaxed text-primary-foreground">
        <MessagePrimitive.Parts />
      </div>
    </MessagePrimitive.Root>
  );
}

const StatisticalPlanCard: DataMessagePartComponent = ({ data }) => {
  const { t } = useTranslation();
  const plan = typeof data === "object" && data !== null ? (data as Record<string, unknown>) : {};
  const researchQuestion =
    typeof plan.researchQuestion === "string" ? plan.researchQuestion : t("panel.assistantPlan");
  const analysisMode = typeof plan.analysisMode === "string" ? plan.analysisMode : "—";
  const workflow = typeof plan.selectedWorkflow === "string" ? plan.selectedWorkflow : "—";
  return (
    <section className="my-2 rounded-md border border-border bg-muted/40 p-2.5">
      <div className="text-[0.6875rem] font-semibold tracking-wide text-muted-foreground uppercase">
        {t("panel.assistantPlan")}
      </div>
      <p className="mt-1 text-xs/relaxed font-medium">{researchQuestion}</p>
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-[0.6875rem] leading-4">
        <dt className="text-muted-foreground">{t("panel.assistantPlanMode")}</dt>
        <dd>{analysisMode}</dd>
        <dt className="text-muted-foreground">{t("panel.assistantPlanWorkflow")}</dt>
        <dd className="truncate font-mono">{workflow}</dd>
      </dl>
    </section>
  );
};

const ToolCallCard: ToolCallMessagePartComponent = ({ toolName, result, isError }) => {
  const { t } = useTranslation();
  const state = isError
    ? t("panel.assistantToolFailed")
    : result === undefined
      ? t("panel.assistantToolRunning")
      : t("panel.assistantToolCompleted");
  return (
    <div className="my-2 flex items-center gap-2 rounded-md border border-border bg-muted/30 px-2.5 py-2 text-[0.6875rem]">
      <span className="min-w-0 flex-1 truncate font-mono">{toolName}</span>
      <span className="shrink-0 text-muted-foreground">{state}</span>
    </div>
  );
};

function memoryLabel(value: Readonly<Record<string, unknown>>): string {
  const payload =
    typeof value.payload === "object" && value.payload !== null
      ? (value.payload as Record<string, unknown>)
      : {};
  for (const key of ["question", "meaning", "rationale", "summary", "value"]) {
    if (typeof payload[key] === "string") return payload[key];
  }
  return typeof value.type === "string" ? value.type : "Memory";
}

function AssistantMessage() {
  return (
    <MessagePrimitive.Root className="flex justify-start py-2">
      <div className="max-w-[92%] rounded-lg border border-border bg-background/70 px-3 py-2 text-xs/relaxed text-foreground shadow-xs">
        <MessagePrimitive.Parts
          components={{
            data: { by_name: { "statistical-plan": StatisticalPlanCard } },
            tools: { Fallback: ToolCallCard },
          }}
        />
      </div>
    </MessagePrimitive.Root>
  );
}

export function AssistantThread() {
  const { t } = useTranslation();
  const snapshot = useAssistantHarnessSnapshot();
  const { deleteMemory } = useAssistantHarnessActions();
  const statusText =
    snapshot.status === "initializing"
      ? t("panel.assistantStatusInitializing")
      : snapshot.status === "provider-unavailable"
        ? t("panel.assistantStatusProviderUnavailable")
        : snapshot.status === "ready" && snapshot.isRunning
          ? t("panel.assistantStatusRunning")
          : snapshot.status === "ready"
            ? t("panel.assistantStatusReady")
            : t("panel.assistantStatusError");

  return (
    <ThreadPrimitive.Root className="flex h-full min-h-0 flex-col bg-(--workbench-bg)">
      <ThreadPrimitive.ViewportProvider>
        <ScrollArea className="min-h-0 flex-1" orientation="vertical">
          <div className="flex min-h-full min-w-0 flex-col p-3">
            <AuiIf condition={(state) => state.thread.isEmpty}>
              <Empty className="min-h-56 flex-1 px-4 py-8">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <VscSparkle aria-hidden />
                  </EmptyMedia>
                  <EmptyTitle>{t("panel.assistantEmptyTitle")}</EmptyTitle>
                  <EmptyDescription>{t("panel.assistantEmptyDescription")}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            </AuiIf>
            <ThreadPrimitive.Messages
              components={{ UserMessage, AssistantMessage, SystemMessage: AssistantMessage }}
            />
          </div>
        </ScrollArea>

        <Separator />
        <div className="shrink-0 p-2">
          <ComposerPrimitive.Root className="rounded-md border border-border bg-background/80 shadow-xs focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/20">
            <ComposerPrimitive.Input
              aria-label={t("panel.assistantComposerLabel")}
              className="max-h-32 min-h-16 w-full resize-none bg-transparent px-2.5 py-2 text-xs/relaxed outline-none placeholder:text-muted-foreground"
              placeholder={t("panel.assistantComposerPlaceholder")}
              submitMode="ctrlEnter"
            />
            <Separator />
            <div className="flex min-w-0 items-center gap-2 px-2 py-1.5">
              {snapshot.memoryCount > 0 ? (
                <Popover>
                  <PopoverTrigger asChild>
                    <Button
                      type="button"
                      size="xs"
                      variant="ghost"
                      className="h-6 px-1.5 text-[0.625rem]"
                    >
                      {t("panel.assistantMemoryCount", { count: snapshot.memoryCount })}
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="start" side="top" className="w-72 gap-2 p-2.5">
                    <div className="text-xs font-semibold">{t("panel.assistantMemoryTitle")}</div>
                    <div className="max-h-52 space-y-1 overflow-y-auto">
                      {snapshot.memoryRecords.map((record) => (
                        <div
                          key={record.recordId}
                          className="flex items-start gap-2 rounded border border-border p-2 text-[0.6875rem]"
                        >
                          <div className="min-w-0 flex-1">
                            <div className="truncate text-muted-foreground">{record.kind}</div>
                            <div className="line-clamp-3 leading-4">
                              {memoryLabel(record.value)}
                            </div>
                          </div>
                          <Button
                            type="button"
                            size="icon-xs"
                            variant="ghost"
                            aria-label={t("panel.assistantMemoryDelete")}
                            title={t("panel.assistantMemoryDelete")}
                            onClick={() => void deleteMemory(record.recordId).catch(() => {})}
                          >
                            <VscTrash aria-hidden />
                          </Button>
                        </div>
                      ))}
                    </div>
                  </PopoverContent>
                </Popover>
              ) : null}
              <span className="min-w-0 flex-1 text-[0.6875rem] leading-4 text-muted-foreground">
                {snapshot.activity
                  ? t("panel.assistantActivity", { activity: snapshot.activity })
                  : statusText}
              </span>
              <AuiIf condition={(state) => state.thread.isRunning}>
                <ComposerPrimitive.Cancel asChild>
                  <Button
                    type="button"
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t("panel.assistantCancel")}
                    title={t("panel.assistantCancel")}
                  >
                    <VscDebugStop aria-hidden />
                  </Button>
                </ComposerPrimitive.Cancel>
              </AuiIf>
              <ComposerPrimitive.Send asChild>
                <Button
                  type="submit"
                  size="icon-sm"
                  aria-label={t("panel.assistantSend")}
                  title={t("panel.assistantSend")}
                >
                  <VscSend data-icon="inline-start" aria-hidden />
                </Button>
              </ComposerPrimitive.Send>
            </div>
          </ComposerPrimitive.Root>
        </div>
      </ThreadPrimitive.ViewportProvider>
    </ThreadPrimitive.Root>
  );
}

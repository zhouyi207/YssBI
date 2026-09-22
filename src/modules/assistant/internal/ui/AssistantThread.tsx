import {
  AuiIf,
  ActionBarPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  type DataMessagePartComponent,
  type SourceMessagePartComponent,
} from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import {
  VscArrowDown,
  VscCheck,
  VscCopy,
  VscDebugStop,
  VscReferences,
  VscSend,
  VscSparkle,
  VscTrash,
} from "react-icons/vsc";

import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import { AssistantMarkdown } from "./AssistantMarkdown";
import { AssistantToolCall, AssistantToolGroup } from "./AssistantToolCalls";

function UserMessage() {
  return (
    <MessagePrimitive.Root className="flex min-w-0 justify-end py-3">
      <div className="min-w-0 max-w-[90%] rounded-2xl rounded-br-sm bg-muted px-3.5 py-2.5 text-[13px] leading-7 wrap-anywhere text-foreground">
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
    <section className="my-3 rounded-lg bg-muted/50 p-3">
      <div className="text-[0.6875rem] font-semibold tracking-wide text-muted-foreground uppercase">
        {t("panel.assistantPlan")}
      </div>
      <p className="mt-1 text-[13px] leading-6 font-medium wrap-anywhere">{researchQuestion}</p>
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-[0.6875rem] leading-4">
        <dt className="text-muted-foreground">{t("panel.assistantPlanMode")}</dt>
        <dd>{analysisMode}</dd>
        <dt className="text-muted-foreground">{t("panel.assistantPlanWorkflow")}</dt>
        <dd className="min-w-0 wrap-anywhere">{workflow}</dd>
      </dl>
    </section>
  );
};

const SourceCard: SourceMessagePartComponent = ({ title }) => {
  const { t } = useTranslation();
  return (
    <div className="my-2 flex min-w-0 items-start gap-2 text-xs leading-5 text-muted-foreground">
      <VscReferences className="mt-1 shrink-0" aria-hidden />
      <span className="wrap-anywhere">
        {t("panel.assistantSource")}: {title}
      </span>
    </div>
  );
};

function MessageActions() {
  const { t } = useTranslation();
  return (
    <ActionBarPrimitive.Root hideWhenRunning className="mt-2 flex items-center">
      <ActionBarPrimitive.Copy asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={t("panel.assistantCopy")}
          title={t("panel.assistantCopy")}
        >
          <AuiIf condition={(s) => s.message.isCopied}>
            <VscCheck aria-hidden />
          </AuiIf>
          <AuiIf condition={(s) => !s.message.isCopied}>
            <VscCopy aria-hidden />
          </AuiIf>
        </Button>
      </ActionBarPrimitive.Copy>
    </ActionBarPrimitive.Root>
  );
}

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
  const { t } = useTranslation();
  return (
    <MessagePrimitive.Root className="min-w-0 py-4">
      <div className="mb-2 flex items-center gap-2 text-xs font-medium text-muted-foreground">
        <VscSparkle aria-hidden />
        {t("panel.assistant")}
      </div>
      <div className="min-w-0 text-foreground">
        <MessagePrimitive.Parts
          components={{
            Text: AssistantMarkdown,
            Source: SourceCard,
            data: { by_name: { "statistical-plan": StatisticalPlanCard } },
            tools: { Fallback: AssistantToolCall },
            ToolGroup: AssistantToolGroup,
          }}
        />
        <MessageActions />
        <AuiIf
          condition={(state) =>
            state.message.status?.type === "incomplete" && state.message.status.reason === "error"
          }
        >
          <p className="mt-3 text-sm leading-relaxed text-muted-foreground">
            {t("panel.assistantReplyInterrupted")}
          </p>
        </AuiIf>
        <AuiIf
          condition={(state) =>
            state.message.status?.type === "incomplete" &&
            state.message.status.reason === "cancelled"
          }
        >
          <p className="mt-3 text-sm leading-relaxed text-muted-foreground">
            {t("panel.assistantReplyStopped")}
          </p>
        </AuiIf>
      </div>
    </MessagePrimitive.Root>
  );
}

export function AssistantThread() {
  const { t } = useTranslation();
  const snapshot = useAssistantHarnessSnapshot();
  const { deleteMemory, newConversation, selectConversation, reloadConversations } =
    useAssistantHarnessActions();
  const statusError =
    snapshot.status === "ready" &&
    snapshot.messages[snapshot.messages.length - 1]?.status.type === "incomplete"
      ? null
      : snapshot.error;
  const statusText = statusError
    ? t(`panel.assistantErrors.${statusError.code}`, {
        defaultValue: t("panel.assistantStatusError"),
      })
    : snapshot.status === "initializing"
      ? t("panel.assistantStatusInitializing")
      : snapshot.status === "provider-unavailable"
        ? t("panel.assistantStatusProviderUnavailable")
        : snapshot.status === "ready" && snapshot.isRunning
          ? t("panel.assistantStatusRunning")
          : snapshot.status === "ready"
            ? t("panel.assistantStatusReady")
            : t("panel.assistantStatusError");

  return (
    <ThreadPrimitive.Root className="flex h-full min-h-0 min-w-0 flex-col bg-(--workbench-bg)">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
        <select
          aria-label={t("panel.assistantConversations")}
          className="h-8 min-w-0 flex-1 rounded-md border border-border bg-background px-2 text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
          value={snapshot.sessionId ?? ""}
          disabled={snapshot.status === "initializing" || snapshot.isRunning}
          onChange={(event) => void selectConversation(event.target.value)}
        >
          {!snapshot.sessionId ? (
            <option value="">{t("panel.assistantConversations")}</option>
          ) : null}
          {snapshot.conversations.map((session) => (
            <option key={session.sessionId} value={session.sessionId}>
              {session.title || t("panel.assistantNewConversation")}
            </option>
          ))}
        </select>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          disabled={
            snapshot.status === "initializing" ||
            snapshot.isRunning ||
            (snapshot.sessionId !== null && snapshot.messages.length === 0)
          }
          onClick={() => void newConversation()}
        >
          {t("panel.assistantNewConversation")}
        </Button>
        {snapshot.status === "error" ? (
          <Button
            type="button"
            variant="outline"
            size="xs"
            onClick={() => void reloadConversations()}
          >
            {t("panel.assistantReloadConversations")}
          </Button>
        ) : null}
      </div>
      <ThreadPrimitive.ViewportProvider>
        <div className="relative flex min-h-0 flex-1 flex-col">
          <ThreadPrimitive.Viewport
            className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain"
            autoScroll
          >
            <div className="mx-auto flex min-h-full w-full min-w-0 max-w-3xl flex-col px-4 py-3">
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
              <ThreadPrimitive.Messages>
                {({ message }) =>
                  message.role === "user" ? <UserMessage /> : <AssistantMessage />
                }
              </ThreadPrimitive.Messages>
            </div>
          </ThreadPrimitive.Viewport>
          <ThreadPrimitive.ScrollToBottom asChild>
            <Button
              type="button"
              variant="outline"
              size="icon-sm"
              className="absolute right-4 bottom-2 rounded-full bg-background shadow-sm disabled:hidden"
              aria-label={t("panel.assistantScrollToBottom")}
              title={t("panel.assistantScrollToBottom")}
            >
              <VscArrowDown aria-hidden />
            </Button>
          </ThreadPrimitive.ScrollToBottom>
        </div>

        <div className="mx-auto w-full max-w-3xl shrink-0 px-3 pt-1 pb-3">
          <ComposerPrimitive.Root className="rounded-xl border border-border bg-background shadow-xs focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/20">
            <ComposerPrimitive.Input
              aria-label={t("panel.assistantComposerLabel")}
              className="max-h-40 min-h-20 w-full resize-none bg-transparent px-3 py-3 text-[13px] leading-6 outline-none placeholder:text-muted-foreground"
              placeholder={t("panel.assistantComposerPlaceholder")}
              submitMode="ctrlEnter"
            />
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
              <span
                role={statusError ? "alert" : undefined}
                className={`min-w-0 flex-1 text-[0.6875rem] leading-4 ${statusError ? "text-destructive" : "text-muted-foreground"}`}
              >
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
              <AuiIf condition={(state) => !state.thread.isRunning}>
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
              </AuiIf>
            </div>
          </ComposerPrimitive.Root>
        </div>
      </ThreadPrimitive.ViewportProvider>
    </ThreadPrimitive.Root>
  );
}

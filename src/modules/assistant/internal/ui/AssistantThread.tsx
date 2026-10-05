import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import {
  AuiIf,
  ActionBarPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  useAuiState,
  type DataMessagePartComponent,
} from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { VscArrowDown, VscCheck, VscCopy, VscSparkle } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";
import { assistantFailureKey } from "@/features/application/assistant/assistantMessageContent";
import type { HarnessArtifact, HarnessResultReference } from "@/services/assistant/harnessContract";
import { AssistantMarkdown } from "./AssistantMarkdown";
import { AssistantToolCall, AssistantToolGroup } from "./AssistantToolCalls";
import { AssistantTurnModel, AssistantTurnTiming } from "./AssistantExecution";
import { AssistantUserReferences } from "./AssistantReferences";
import { AssistantComposer } from "./AssistantComposer";
import { AgentTaskCard, StatisticalPlanCard } from "./AssistantTasks";
import { AssistantArtifacts, AssistantSourceCard } from "./AssistantResources";

function UserMessage() {
  return (
    <MessagePrimitive.Root className="flex min-w-0 justify-end py-3">
      <div className="min-w-0 max-w-[90%] rounded-2xl rounded-br-sm bg-muted px-3.5 py-2.5 text-[13px] leading-7 wrap-anywhere text-foreground">
        <MessagePrimitive.Parts />
        <AssistantUserReferences />
      </div>
    </MessagePrimitive.Root>
  );
}

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

const AssistantFailure: DataMessagePartComponent = ({ data }) => {
  const { t } = useTranslation();
  const code =
    typeof data === "object" && data !== null && "code" in data && typeof data.code === "string"
      ? data.code
      : "internal_failure";
  return (
    <div className="mt-3 rounded-lg border border-destructive/20 bg-destructive/5 p-3 text-xs leading-6">
      <p>
        {t(`panel.assistantErrors.${assistantFailureKey(code)}`, {
          defaultValue: t("panel.assistantReplyInterrupted"),
        })}
      </p>
      <details className="mt-1 text-muted-foreground">
        <summary className="cursor-pointer">{t("panel.assistantTechnicalDetails")}</summary>
        <code>{code}</code>
      </details>
    </div>
  );
};
const ArtifactCard: DataMessagePartComponent = ({ data }) => (
  <AssistantArtifacts
    {...(data as {
      artifacts: readonly HarnessArtifact[];
      results: readonly HarnessResultReference[];
    })}
  />
);

function AssistantMessage() {
  const { t } = useTranslation();
  return (
    <MessagePrimitive.Root className="min-w-0 py-4">
      <div className="mb-2 flex flex-wrap items-center gap-2 text-xs font-medium text-muted-foreground">
        <VscSparkle aria-hidden />
        {t("panel.assistant")}
        <AssistantTurnTiming />
        <AssistantTurnModel />
      </div>
      <div className="min-w-0 text-foreground">
        <MessagePrimitive.Parts
          components={{
            Text: AssistantMarkdown,
            Source: AssistantSourceCard,
            data: {
              by_name: {
                "statistical-plan": StatisticalPlanCard,
                "agent-task": AgentTaskCard,
                "assistant-failure": AssistantFailure,
                artifacts: ArtifactCard,
              },
            },
            tools: { Fallback: AssistantToolCall },
            ToolGroup: AssistantToolGroup,
          }}
        />
        <MessageActions />
        <AuiIf
          condition={(state) =>
            state.message.status?.type === "incomplete" &&
            state.message.status.reason === "error" &&
            !state.message.content.some(
              (part) => part.type === "data" && part.name === "assistant-failure",
            )
          }
        >
          <p className="mt-3 text-xs leading-6 text-muted-foreground">
            {t("panel.assistantReplyInterrupted")}
          </p>
        </AuiIf>
        <AuiIf
          condition={(state) =>
            state.message.status?.type === "incomplete" &&
            state.message.status.reason === "cancelled"
          }
        >
          <p className="mt-3 text-xs leading-6 text-muted-foreground">
            {t("panel.assistantReplyStopped")}
          </p>
        </AuiIf>
      </div>
    </MessagePrimitive.Root>
  );
}

const MESSAGE_COMPONENTS = { UserMessage, AssistantMessage };
function PagedMessages({ viewport }: { viewport: RefObject<HTMLDivElement | null> }) {
  const { t } = useTranslation();
  const count = useAuiState((state) => state.thread.messages.length);
  const [start, setStart] = useState<number | null>(null);
  const first = start ?? Math.max(0, count - 40);
  const previousScroll = useRef<{ height: number; top: number } | null>(null);
  useEffect(() => {
    if (start === null && count > 0) setStart(first);
  }, [count, first, start]);
  useLayoutEffect(() => {
    if (previousScroll.current && viewport.current) {
      viewport.current.scrollTop =
        previousScroll.current.top + viewport.current.scrollHeight - previousScroll.current.height;
      previousScroll.current = null;
    }
  }, [first, viewport]);
  return (
    <>
      {first > 0 && (
        <Button
          type="button"
          size="xs"
          variant="ghost"
          className="mx-auto my-2"
          onClick={() => {
            if (viewport.current)
              previousScroll.current = {
                height: viewport.current.scrollHeight,
                top: viewport.current.scrollTop,
              };
            setStart(Math.max(0, first - 40));
          }}
        >
          {t("panel.assistantLoadEarlier", { count: first })}
        </Button>
      )}
      {Array.from({ length: Math.max(0, count - first) }, (_, offset) => (
        <ThreadPrimitive.MessageByIndex
          key={first + offset}
          index={first + offset}
          components={MESSAGE_COMPONENTS}
        />
      ))}
    </>
  );
}

export function AssistantThread() {
  const { t } = useTranslation();
  const sessionId = useAssistantHarnessSnapshot((state) => state.sessionId);
  const viewport = useRef<HTMLDivElement>(null);
  return (
    <ThreadPrimitive.Root className="flex h-full min-h-0 min-w-0 flex-col bg-(--workbench-bg)">
      <ThreadPrimitive.ViewportProvider>
        <div className="relative flex min-h-0 flex-1 flex-col">
          <ThreadPrimitive.Viewport
            ref={viewport}
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
              <PagedMessages key={sessionId} viewport={viewport} />
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
        <AssistantComposer />
      </ThreadPrimitive.ViewportProvider>
    </ThreadPrimitive.Root>
  );
}

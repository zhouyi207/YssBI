import { useState } from "react";
import { TextMessagePartProvider, type DataMessagePartComponent } from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { VscChevronRight, VscLoading } from "react-icons/vsc";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  assistantFailureKey,
  type ProjectionAgentTask,
} from "@/features/application/assistant/assistantMessageContent";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";
import { AssistantElapsed } from "./AssistantExecution";
import { AssistantMarkdown } from "./AssistantMarkdown";
import { AssistantToolDetails } from "./AssistantToolCalls";
import { AssistantArtifacts } from "./AssistantResources";

export const StatisticalPlanCard: DataMessagePartComponent = ({ data }) => {
  const { t } = useTranslation();
  const plan = typeof data === "object" && data !== null ? (data as Record<string, unknown>) : {};
  return (
    <section className="my-3 rounded-lg bg-muted/50 p-3 text-xs">
      <div className="font-semibold text-muted-foreground">{t("panel.assistantPlan")}</div>
      <p className="mt-1 text-[13px] leading-6 font-medium wrap-anywhere">
        {typeof plan.researchQuestion === "string"
          ? plan.researchQuestion
          : t("panel.assistantPlan")}
      </p>
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 leading-5">
        <dt className="text-muted-foreground">{t("panel.assistantPlanMode")}</dt>
        <dd>{typeof plan.analysisMode === "string" ? plan.analysisMode : "—"}</dd>
        <dt className="text-muted-foreground">{t("panel.assistantPlanWorkflow")}</dt>
        <dd className="min-w-0 wrap-anywhere">
          {typeof plan.selectedWorkflow === "string" ? plan.selectedWorkflow : "—"}
        </dd>
      </dl>
    </section>
  );
};

export const AgentTaskCard: DataMessagePartComponent = ({ data }) => {
  const { t } = useTranslation();
  const task = data as ProjectionAgentTask;
  const [expanded, setExpanded] = useState<boolean | undefined>();
  const running = task.state === "running";
  const connected = useAssistantHarnessSnapshot(
    (state) => state.status !== "error" && state.status !== "initializing" && state.isRunning,
  );
  const failed = ["failed", "blocked", "interrupted"].includes(task.state);
  return (
    <div className="my-2 min-w-0 rounded-lg border border-border/60 bg-muted/20 text-xs">
      <Collapsible open={expanded ?? (running || failed)} onOpenChange={setExpanded}>
        <CollapsibleTrigger className="group flex w-full flex-wrap items-center gap-2 rounded-lg px-3 py-2.5 text-left hover:bg-muted/50">
          <VscChevronRight aria-hidden className="shrink-0 group-data-[state=open]:rotate-90" />
          {running && connected && <VscLoading aria-hidden className="motion-safe:animate-spin" />}
          <span className="font-medium">{t(`panel.assistantAgentRoles.${task.role}`)}</span>
          <span className={failed ? "text-destructive" : "text-muted-foreground"}>
            {running && !connected
              ? t("panel.assistantToolUnknown")
              : t(`panel.assistantAgentStates.${task.state}`)}
          </span>
          <AssistantElapsed
            startedAt={task.startedAt}
            completedAt={task.finishedAt}
            updatedAt={task.updatedAt}
            running={running}
          />
          <span className="line-clamp-2 w-full wrap-anywhere text-muted-foreground">
            {task.objective}
          </span>
        </CollapsibleTrigger>
        {task.activity && connected && (
          <p className="px-3 pb-2 text-muted-foreground" role="status">
            {t(`panel.assistantToolNames.${task.activity}`, { defaultValue: task.activity })}
            {task.activity === "compacting" &&
              task.compactionProgress !== null &&
              ` ${task.compactionProgress}%`}
            {task.activity === "reconnecting" &&
              task.recoveryAttempt > 0 &&
              ` · ${t("panel.assistantRecoveryAttempt", { count: task.recoveryAttempt })}`}
          </p>
        )}
        {task.failureCode && (
          <p className="px-3 pb-2 text-destructive">
            {t(`panel.assistantErrors.${assistantFailureKey(task.failureCode)}`, {
              defaultValue: task.failureCode,
            })}
          </p>
        )}
        {task.blockedReason && (
          <p className="px-3 pb-2 text-destructive">
            {t(`panel.assistantBlockedReasons.${task.blockedReason}`, {
              defaultValue: task.blockedReason,
            })}
          </p>
        )}
        <CollapsibleContent className="space-y-2 border-t border-border/60 p-3">
          <p className="wrap-anywhere text-muted-foreground">{task.objective}</p>
          {task.tools.length > 0 && (
            <div className="min-w-0">
              {task.tools.map((part) =>
                part.type === "tool-call" && part.toolCallId ? (
                  <AssistantToolDetails
                    key={part.toolCallId}
                    toolCallId={part.toolCallId}
                    toolName={part.toolName ?? ""}
                    result={part.result}
                    isError={part.isError}
                    timing={part.timing}
                  />
                ) : null,
              )}
            </div>
          )}
          {task.summary && (
            <TextMessagePartProvider text={task.summary}>
              <AssistantMarkdown />
            </TextMessagePartProvider>
          )}
          {task.plan !== null && (
            <StatisticalPlanCard
              data={task.plan}
              name="statistical-plan"
              type="data"
              status={{ type: "complete" }}
            />
          )}
          {task.warnings.map((warning, index) => (
            <p key={index} className="wrap-anywhere text-muted-foreground">
              {warning}
            </p>
          ))}
        </CollapsibleContent>
      </Collapsible>
      {(task.artifacts.length > 0 || task.results.length > 0) && (
        <div className="px-3 pb-1">
          <AssistantArtifacts artifacts={task.artifacts} results={task.results} />
        </div>
      )}
    </div>
  );
};

import { useTranslation } from "react-i18next";
import { FiTrash2 } from "react-icons/fi";
import { ScrollArea } from "@/components/ui/scroll-area";
import { runOutputActions } from "@/features/core/execution";
import { useExecutionRead } from "@/features/core/execution/read";
import { useGraphSessionUi } from "@/features/core/graphSession/ui";
import { revealGraphProblem } from "@/features/application/editor/revealGraphProblem";
import type { RunOutputProjection } from "@/features/core/execution/executionTypes";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";

const EMPTY_RUN_OUTPUT: RunOutputProjection = {
  runId: null,
  entries: [],
  projectionDropped: false,
};

function formatRunOutputSource(entry: RunOutputProjection["entries"][number]): string {
  const sourcePortLabel =
    entry.sourcePort.kind === "declared"
      ? entry.sourcePort.portKey
      : `${entry.sourcePort.templateKey}[${entry.sourcePort.instanceId}]`;
  return `${entry.sourceGraphPath} · ${entry.sourceNodeId} · ${sourcePortLabel}`;
}

export function RunOutputPanel() {
  const { t } = useTranslation();
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  const graphPath = focusedSession?.graphPath ?? null;
  const runOutput = useExecutionRead((snapshot) =>
    graphPath ? (snapshot.graphs[graphPath]?.runOutput ?? EMPTY_RUN_OUTPUT) : EMPTY_RUN_OUTPUT,
  );
  const clearRunOutput = runOutputActions.clearRunOutput;
  const failure = useExecutionRead((snapshot) =>
    graphPath ? (snapshot.graphs[graphPath]?.runFailure ?? null) : null,
  );
  const failureSource = failure?.source;
  const hasOutput = runOutput.entries.length > 0 || runOutput.projectionDropped;

  return (
    <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
      <div
        data-output-panel-header
        className="flex h-(--panel-toolbar-height) shrink-0 items-center justify-between gap-1 border-b border-border/20 bg-background px-1"
      >
        <span className="min-w-0 truncate px-1 text-xs font-medium text-foreground">
          {t("panel.output")}
        </span>
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          disabled={!graphPath || (!hasOutput && !failure)}
          onClick={() => {
            if (graphPath) clearRunOutput(graphPath);
          }}
          tooltip={t("panel.outputClear")}
          aria-label={t("panel.outputClear")}
        >
          <FiTrash2 />
        </ToolbarIconButton>
      </div>

      {!graphPath ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t("panel.outputNoGraph")}
        </div>
      ) : !hasOutput && !failure ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t("panel.outputEmpty")}
        </div>
      ) : (
        <ScrollArea orientation="both" className="min-h-0 flex-1">
          {failure ? (
            <section
              role="alert"
              className="space-y-2 border-b border-destructive/30 bg-destructive/5 p-3 text-xs"
            >
              <p className="font-medium text-destructive">{t("runFailure.title")}</p>
              <p>
                {t(`runFailure.causes.${failure.code}`, { defaultValue: t("runFailure.unknown") })}
              </p>
              {failureSource?.nodeId ? (
                <button
                  type="button"
                  className="block text-left text-primary underline-offset-2 enabled:hover:underline disabled:text-muted-foreground"
                  disabled={!focusedSession}
                  title={failureSource.nodeId}
                  onClick={() => {
                    if (focusedSession && failureSource.nodeId) {
                      void revealGraphProblem(
                        failureSource.graphPath,
                        { kind: "node", nodeId: failureSource.nodeId },
                        focusedSession.groupId,
                      );
                    }
                  }}
                >
                  {t("runFailure.node", {
                    name: failureSource.nodeId,
                  })}
                </button>
              ) : null}
              <div className="flex flex-wrap gap-x-3 gap-y-1 text-muted-foreground">
                {failure.phase ? <span>{t(`runFailure.phases.${failure.phase}`)}</span> : null}
                {failure.runId ? <span>{t("runFailure.run", { id: failure.runId })}</span> : null}
                <span>{t("runFailure.code", { code: failure.code })}</span>
                {failure.incidentId ? (
                  <span>{t("runFailure.incident", { id: failure.incidentId })}</span>
                ) : null}
              </div>
            </section>
          ) : null}
          <div className="min-w-max py-1 font-mono text-xs" role="log" aria-live="polite">
            {runOutput.projectionDropped ? (
              <div className="px-3 py-1.5 text-amber-500">{t("panel.outputProjectionDropped")}</div>
            ) : null}
            {runOutput.entries.map((entry) => {
              const sourceLabel = formatRunOutputSource(entry);

              return (
                <div
                  key={`${entry.runId}:${entry.sequence}`}
                  className="grid grid-cols-[4rem_4rem_minmax(10rem,1fr)] items-start gap-2 border-b border-border/10 px-3 py-1.5 last:border-b-0"
                >
                  <span className="text-right text-muted-foreground">{entry.sequence}</span>
                  <span className={entry.stream === "stderr" ? "text-destructive" : "text-primary"}>
                    {entry.stream}
                  </span>
                  <div className="min-w-0">
                    {"text" in entry ? (
                      <pre className="whitespace-pre-wrap wrap-break-word text-foreground">
                        {entry.text}
                      </pre>
                    ) : (
                      <span className="text-amber-500">
                        {t(
                          entry.status === "truncated"
                            ? "panel.outputTruncated"
                            : "panel.outputDropped",
                        )}
                      </span>
                    )}
                    <div
                      className="truncate text-[10px] text-muted-foreground/70"
                      title={sourceLabel}
                    >
                      {t("panel.outputSource")}: {sourceLabel}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </ScrollArea>
      )}
    </div>
  );
}

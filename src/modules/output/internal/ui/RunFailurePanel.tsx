import { useTranslation } from "react-i18next";
import { FiTrash2 } from "react-icons/fi";
import { ScrollArea } from "@/components/ui/scroll-area";
import { runFailureActions } from "@/features/core/execution";
import { useExecutionRead } from "@/features/core/execution/read";
import { useActiveGraphContext } from "@/features/application/editor/editorGroupContext";
import { revealGraphProblem } from "@/features/application/editor/revealGraphProblem";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";

export function RunFailurePanel() {
  const { t } = useTranslation();
  const activeGraph = useActiveGraphContext();
  const graphPath = activeGraph?.graphPath ?? null;
  const clearRunFailure = runFailureActions.clearRunFailure;
  const failure = useExecutionRead((snapshot) =>
    graphPath ? (snapshot.graphs[graphPath]?.runFailure ?? null) : null,
  );
  const failureSource = failure?.source;

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
          disabled={!graphPath || !failure}
          onClick={() => {
            if (graphPath) clearRunFailure(graphPath);
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
      ) : !failure ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t("panel.outputEmpty")}
        </div>
      ) : (
        <ScrollArea orientation="both" className="min-h-0 flex-1">
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
                disabled={!activeGraph}
                title={failureSource.nodeId}
                onClick={() => {
                  if (activeGraph && failureSource.nodeId) {
                    void revealGraphProblem(
                      failureSource.graphPath,
                      { kind: "node", nodeId: failureSource.nodeId },
                      activeGraph.groupId,
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
        </ScrollArea>
      )}
    </div>
  );
}

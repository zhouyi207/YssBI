import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscError, VscInfo, VscWarning } from "react-icons/vsc";
import { ScrollArea } from "@/components/ui/scroll-area";
import { revealGraphProblem } from "@/features/application/editor/revealGraphProblem";
import { useGraphRead } from "@/features/core/graph/read";
import { useGraphSessionUi } from "@/features/core/graphSession/ui";
import type { FocusedGraphSession } from "@/features/core/graphSession/graphSessionStore";
import {
  collectGraphProblems,
  formatDiagnosticLocationLabel,
  formatGraphDiagnostic,
} from "@/features/domain/graphDiagnostics/nodeDiagnostics";
import type { GraphProblem } from "@/features/domain/graphDiagnostics/nodeDiagnostics";

function severityIcon(severity: GraphProblem["diagnostic"]["severity"]) {
  if (severity === "error") return VscError;
  if (severity === "warning") return VscWarning;
  return VscInfo;
}

function severityClass(severity: GraphProblem["diagnostic"]["severity"]): string {
  if (severity === "error") return "text-destructive";
  if (severity === "warning") return "text-amber-500";
  return "text-muted-foreground";
}

function activateProblem(problem: GraphProblem, focusedSession: FocusedGraphSession | null): void {
  if (!focusedSession || focusedSession.graphPath !== problem.graphPath) return;

  void revealGraphProblem(problem.graphPath, problem.diagnostic.location, focusedSession.groupId);
}

export function GraphProblemsPanel() {
  const { t, i18n } = useTranslation();
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  const graphPath = focusedSession?.graphPath ?? null;
  const graphProjection = useGraphRead((snapshot) =>
    graphPath ? snapshot.graphEntities[graphPath] : undefined,
  );
  const problems = useMemo(
    () => collectGraphProblems(graphPath ?? "", graphProjection),
    [graphPath, graphProjection],
  );

  return (
    <div
      data-graph-problems-panel
      className="flex h-full min-h-0 flex-col bg-background text-foreground"
    >
      <div
        data-graph-problems-panel-header
        className="flex h-(--panel-toolbar-height) shrink-0 items-center justify-between gap-1 border-b border-border/20 bg-background px-1"
      >
        <span className="min-w-0 truncate px-1 text-xs font-medium text-foreground">
          {t("panel.problems")}
        </span>
        <span className="shrink-0 px-1 text-[11px] text-muted-foreground">
          {t("panel.problemsCount", { count: problems.length })}
        </span>
      </div>

      {!graphPath ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t("panel.problemsNoGraph")}
        </div>
      ) : problems.length === 0 ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t("panel.problemsEmpty")}
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <div className="py-1" role="list" aria-label={t("panel.problems")}>
            {problems.map((problem, index) => {
              const Icon = severityIcon(problem.diagnostic.severity);
              const iconClass = severityClass(problem.diagnostic.severity);
              return (
                <div
                  key={`${problem.diagnostic.code}:${problem.locationLabel}:${index}`}
                  role="listitem"
                >
                  <button
                    type="button"
                    data-graph-problem-row
                    className="flex w-full items-start gap-2 border-b border-border/10 px-3 py-2 text-left transition-colors enabled:hover:bg-accent/40 enabled:focus-visible:bg-accent/40 enabled:focus-visible:outline-none disabled:cursor-default"
                    onClick={() => activateProblem(problem, focusedSession)}
                    title={t("panel.problemsLocate")}
                  >
                    <Icon className={`mt-0.5 size-3.5 shrink-0 ${iconClass}`} aria-hidden />
                    <span className="min-w-0 flex-1">
                      <span className="flex min-w-0 items-baseline gap-2">
                        <span
                          className="truncate text-xs font-medium"
                          title={problem.locationLabel}
                        >
                          {problem.locationLabel}
                        </span>
                        <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                          {problem.diagnostic.code}
                        </span>
                        {problem.diagnostic.blocking && (
                          <span className="text-[10px] text-muted-foreground">
                            {t("panel.problemsBlocking")}
                          </span>
                        )}
                      </span>
                      <span className="block break-words text-xs text-foreground">
                        {formatGraphDiagnostic(problem.diagnostic, i18n?.resolvedLanguage)}
                      </span>
                    </span>
                  </button>
                  {problem.diagnostic.related.map((location, relatedIndex) => (
                    <button
                      key={relatedIndex}
                      type="button"
                      className="block w-full px-8 py-1 text-left text-xs text-primary hover:underline"
                      data-graph-problem-related
                      onClick={() => {
                        if (focusedSession)
                          void revealGraphProblem(
                            problem.graphPath,
                            location,
                            focusedSession.groupId,
                          );
                      }}
                    >
                      {t("panel.problemsRelated")}:{" "}
                      {formatDiagnosticLocationLabel(location, graphProjection, null) ??
                        problem.graphPath}
                    </button>
                  ))}
                </div>
              );
            })}
          </div>
        </ScrollArea>
      )}
    </div>
  );
}

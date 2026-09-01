import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { ScrollArea } from "@/components/ui/scroll-area";

import type {
  BayesDatasetSelectionDTO,
  BayesInferenceTaskDTO,
  ValidationReportDTO,
} from "@/shared/types/bayes";
import {
  useBayesDatasets,
  useBayesInferenceTask,
  useBayesModelDraft,
  useBayesValidation,
} from "@/features/application/bayes";
import type { BayesArtifactsModel, BayesDatasetOption } from "@/features/application/bayes";
import type { BayesInferenceError } from "@/features/application/bayes";
import { issueTargetStep } from "@/features/domain/bayes";
import { useProjectSync } from "@/features/application/initialization";
import { initializeProjectForCurrentWindow } from "@/features/application/project";
import {
  useCurrentWindowActions,
  useCustomTitleBar,
  usePersistedWindow,
} from "@/features/application/window";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { WindowChromeControls } from "@/shared/ui/WindowChromeControls";
import { WindowMenuBar } from "@/shared/ui/WindowChrome";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { FormulaStep } from "./components/model/FormulaStep";
import { SamplerStep } from "./components/model/SamplerStep";
import { SymbolRoleStep } from "./components/model/SymbolRoleStep";
import { ResultOverview } from "./components/BayesResultPanels";
import { BayesProgressStatus } from "./components/BayesProgressStatus";
import { bayesInferenceErrorMessage, bayesValidationIssueMessage } from "./bayesIssuePresentation";
import { bayesErrorReferenceMessage } from "./bayesIssuePresentation";

export function BayesView() {
  const { t } = useTranslation();
  const windowActions = useCurrentWindowActions();
  const customChrome = useCustomTitleBar();

  usePersistedWindow("bayes");
  useProjectSync();

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await initializeProjectForCurrentWindow();
        if (cancelled) return;
        await windowActions.show();
      } catch (error) {
        reportViewIssue("app", error, "BayesView");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [windowActions]);

  const datasetsModel = useBayesDatasets();
  const datasets = datasetsModel.datasets;
  const modelDraft = useBayesModelDraft();

  useEffect(() => {
    const currentDatasetId = modelDraft.draft.dataset?.sourceId;
    const currentDataset = datasets.find((dataset) => dataset.sourceId === currentDatasetId);
    const nextDataset = currentDataset ?? datasets[0] ?? null;
    if (!nextDataset) return;
    if (!currentDataset || !sameBayesDataset(currentDataset, modelDraft.draft.dataset)) {
      modelDraft.updateDataset({
        sourceType: nextDataset.sourceType,
        sourceId: nextDataset.sourceId,
        columns: nextDataset.columns.map((column) => ({
          name: column.name,
          dtype: column.dtype,
          nullable: column.nullable,
        })),
      });
    }
  }, [datasets, modelDraft.draft.dataset, modelDraft.updateDataset]);
  const validation = useBayesValidation(modelDraft.draft, modelDraft.draftHash);
  const inference = useBayesInferenceTask();
  const applicationIssue = datasetsModel.issue ?? inference.error ?? validation.error;
  const symbolIssues =
    validation.stale || !validation.report
      ? []
      : [...validation.report.errors, ...validation.report.warnings].filter((issue) =>
          ["data", "likelihood", "parameters"].includes(issueTargetStep(issue)),
        );
  const run = async () => {
    const report = await validation.validate();
    if (!report?.ok) return;
    await inference.run(modelDraft.draft);
  };

  return (
    <div
      className="flex h-screen min-h-0 flex-col bg-background text-foreground"
      data-yssbi-workbench
    >
      <WindowMenuBar
        customChrome={customChrome}
        windowActions={
          <WindowChromeControls
            maximized={windowActions.maximized}
            minimize={windowActions.minimize}
            toggleMaximize={windowActions.toggleMaximize}
            close={windowActions.close}
          />
        }
      >
        <div className="flex items-center gap-2 px-4 pointer-events-none self-center">
          <div className="flex size-5 items-center justify-center rounded bg-(--accent-color)">
            <span className="text-white font-black text-xs">B</span>
          </div>
          <div className="text-foreground font-bold text-sm tracking-tight">{t("bayes.title")}</div>
        </div>
      </WindowMenuBar>
      <Tabs defaultValue="model" className="min-h-0 flex-1 gap-0">
        <section className="flex items-center justify-between gap-4 border-b border-border px-6 py-3">
          <TabsList className="grid w-full max-w-md grid-cols-2">
            <TabsTrigger value="model">{t("bayes.tabs.model")}</TabsTrigger>
            <TabsTrigger value="results">{t("bayes.tabs.results")}</TabsTrigger>
          </TabsList>
          <BayesActionBar
            validationLoading={validation.loading}
            phase={inference.phase}
            task={inference.task}
            onRun={run}
            onCancel={inference.cancel}
          />
        </section>
        <ScrollArea className="min-h-0 flex-1">
          <main className="p-6">
            <TabsContent value="model">
              <section className="space-y-4">
                <BayesIssueBanner error={applicationIssue} validation={null} />
                <FormulaStep
                  draft={modelDraft.draft}
                  error={modelDraft.formulaError}
                  onErrorClear={modelDraft.clearFormulaError}
                  onModelEquationChange={modelDraft.updateModelEquation}
                />
                <SymbolRoleStep
                  draft={modelDraft.draft}
                  datasets={datasets}
                  issues={symbolIssues}
                  onSymbolConfigurationChange={modelDraft.updateSymbolConfiguration}
                />
                <SamplerStep draft={modelDraft.draft} onSamplerChange={modelDraft.updateSampler} />
              </section>
            </TabsContent>
            <TabsContent value="results">
              <BayesIssueBanner error={applicationIssue} validation={validation.report} />
              <ResultOverview result={inference.result} />
            </TabsContent>
          </main>
        </ScrollArea>
      </Tabs>
    </div>
  );
}

function sameBayesDataset(
  left: BayesDatasetOption,
  right: BayesDatasetSelectionDTO | null,
): boolean {
  if (!right) return false;
  if (left.sourceId !== right.sourceId || left.sourceType !== right.sourceType) return false;
  if (left.columns.length !== right.columns.length) return false;
  return left.columns.every((column, index) => {
    const other = right.columns[index];
    return (
      other &&
      column.name === other.name &&
      column.dtype === other.dtype &&
      column.nullable === other.nullable
    );
  });
}

function BayesIssueBanner({
  error,
  validation,
}: {
  error: BayesInferenceError | BayesArtifactsModel["issue"];
  validation: ValidationReportDTO | null;
}) {
  const { t } = useTranslation();
  const issues = validation ? [...validation.errors, ...validation.warnings].slice(0, 4) : [];
  if (!error && issues.length === 0) return null;

  const destructive = Boolean(error || issues.some((issue) => issue.severity === "error"));
  return (
    <Alert variant={destructive ? "destructive" : "warning"}>
      <AlertTitle>{error ? t("bayes.errors.title") : t("bayes.validation.title")}</AlertTitle>
      <AlertDescription>
        {error ? (
          <div className="space-y-1">
            <p>
              <span className="font-mono">[{error.code}]</span>{" "}
              {"details" in error
                ? bayesInferenceErrorMessage(error, t)
                : bayesErrorReferenceMessage(error, t)}
            </p>
            {"details" in error && error.details?.column ? (
              <p>{t("bayes.issue.column", { column: error.details.column })}</p>
            ) : null}
            {"details" in error && typeof error.details?.row === "number" ? (
              <p>{t("bayes.issue.row", { row: error.details.row + 1 })}</p>
            ) : null}
            {"details" in error && error.details?.parameter ? (
              <p>{t("bayes.issue.parameter", { parameter: error.details.parameter })}</p>
            ) : null}
            {"details" in error && error.details?.path ? (
              <p>{t("bayes.issue.path", { path: error.details.path })}</p>
            ) : null}
            {error.incidentId ? (
              <p>
                {t("common.incidentId")}: <span className="font-mono">{error.incidentId}</span>
              </p>
            ) : null}
          </div>
        ) : null}

        {issues.map((issue) => (
          <p key={`${issue.code}-${issue.path}`}>
            <span className="font-mono">[{issue.code}]</span>{" "}
            {bayesValidationIssueMessage(issue, t)}{" "}
            <span className="text-xs">({t("bayes.validation.path", { path: issue.path })})</span>
          </p>
        ))}
      </AlertDescription>
    </Alert>
  );
}

function BayesActionBar({
  validationLoading,
  phase,
  task,
  onRun,
  onCancel,
}: {
  validationLoading: boolean;
  phase: ReturnType<typeof useBayesInferenceTask>["phase"];
  task: BayesInferenceTaskDTO | null;
  onRun: () => void | Promise<unknown>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const taskStatus = task?.status ?? null;
  const cancellable =
    taskStatus === "queued" || taskStatus === "running" || taskStatus === "cancelling";
  const busy = cancellable || phase === "submitting" || phase === "reading_result";
  const stageOverride = phase === "reading_result" ? "rendering_result" : undefined;
  return (
    <div className="flex shrink-0 items-center gap-3">
      {busy && task ? <BayesProgressStatus task={task} stageOverride={stageOverride} /> : null}
      <Button size="sm" onClick={onRun} disabled={busy || validationLoading}>
        {busy ? t("bayes.actions.running") : t("bayes.actions.run")}
      </Button>
      <Button size="sm" variant="outline" onClick={onCancel} disabled={!cancellable}>
        {t("bayes.actions.cancel")}
      </Button>
    </div>
  );
}

export default BayesView;

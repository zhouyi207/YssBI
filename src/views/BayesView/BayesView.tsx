import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';

import type { BayesDatasetSelectionDTO, BayesInferenceTaskDTO, ValidationReportDTO } from '@/shared/types/bayes';
import { useBayesInferenceTask, useBayesModelDraft, useBayesValidation } from '@/features/application/bayes';
import type { BayesInferenceError } from '@/features/application/bayes';
import { issueTargetStep } from '@/features/domain/bayes';
import { useProjectSync } from '@/features/application/initialization';
import { initProjectSync, useDatabaseStore } from '@/features/core/dataStore';
import { DatabaseService } from '@/services/database/databaseService';
import { usePersistedWindow, useWindowMaximized } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowMenuBar } from '@/shared/ui/WindowChrome';

import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { FormulaStep, SamplerStep, SymbolRoleStep, type BayesDatasetOption } from './components/BayesPanels';
import { ResultOverview } from './components/BayesResultPanels';
import { BayesProgressStatus } from './components/BayesProgressStatus';

export function BayesView() {
  const { t } = useTranslation();
  const isMaximized = useWindowMaximized('BayesView');

  usePersistedWindow('bayes');
  useProjectSync();

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await initProjectSync();
        if (cancelled) return;
        await getCurrentWindow().show();
      } catch (error) {
        logger.app.error(String(error), 'BayesView');
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const databases = useDatabaseStore(state => state.databases);
  const updateDatabase = useDatabaseStore(state => state.updateDatabase);
  const datasets = useMemo<BayesDatasetOption[]>(
    () => Object.values(databases).map(database => ({
      sourceType: 'table' as const,
      sourceId: database.id,
      displayName: database.name,
      columns: (database.columns ?? []).map(column => ({
        name: column.name,
        dtype: bayesColumnDType(column.type),
        nullable: true,
      })),
    })),
    [databases],
  );
  const modelDraft = useBayesModelDraft();

  useEffect(() => {
    let cancelled = false;
    for (const database of Object.values(databases)) {
      if ((database.columns?.length ?? 0) > 0) continue;
      const id = database.id;
      void DatabaseService.getDatabaseMeta(id)
        .then((meta) => {
          if (cancelled) return;
          updateDatabase(id, {
            name: meta.name,
            columns: meta.columns,
            rowCount: meta.rowCount,
            columnCount: meta.columnCount,
          });
        })
        .catch((error) => logger.data.warn('getDatabaseMeta failed: ' + String(error), 'BayesView'));
    }
    return () => { cancelled = true; };
  }, [databases, updateDatabase]);

  useEffect(() => {
    const currentDatasetId = modelDraft.draft.dataset?.sourceId;
    const currentDataset = datasets.find(dataset => dataset.sourceId === currentDatasetId);
    const nextDataset = currentDataset ?? datasets[0] ?? null;
    if (!nextDataset) return;
    if (!currentDataset || !sameBayesDataset(currentDataset, modelDraft.draft.dataset)) {
      modelDraft.updateDataset({
        sourceType: nextDataset.sourceType,
        sourceId: nextDataset.sourceId,
        columns: nextDataset.columns,
      });
    }
  }, [datasets, modelDraft.draft.dataset, modelDraft.updateDataset]);
  const validation = useBayesValidation(modelDraft.draft, modelDraft.draftHash);
  const inference = useBayesInferenceTask();
  const symbolIssues = validation.stale || !validation.report
    ? []
    : [...validation.report.errors, ...validation.report.warnings]
      .filter(issue => ['data', 'likelihood', 'parameters'].includes(issueTargetStep(issue)));
  const run = async () => {
    const report = await validation.validate();
    if (!report.ok) return;
    await inference.run(modelDraft.draft);
  };

  return (
    <div className="flex h-screen min-h-0 flex-col bg-background text-foreground" data-yssbi-workbench>
      <WindowMenuBar windowActions={<WindowChromeControls isMaximized={isMaximized} />}>
        <div className="flex items-center gap-2 px-4 pointer-events-none self-center">
          <div className="flex size-5 items-center justify-center rounded bg-(--accent-color)">
            <span className="text-white font-black text-xs">B</span>
          </div>
          <div className="text-foreground font-bold text-sm tracking-tight">
            {t('bayes.title')}
          </div>
        </div>

      </WindowMenuBar>
      <Tabs defaultValue="model" className="min-h-0 flex-1 gap-0">
        <section className="flex items-center justify-between gap-4 border-b border-border px-6 py-3">
          <TabsList className="grid w-full max-w-md grid-cols-2">
            <TabsTrigger value="model">{t('bayes.tabs.model')}</TabsTrigger>
            <TabsTrigger value="results">{t('bayes.tabs.results')}</TabsTrigger>
          </TabsList>
          <BayesActionBar
            validationLoading={validation.loading}
            phase={inference.phase}
            task={inference.task}
            onRun={run}
            onCancel={inference.cancel}
          />
        </section>
        <OverlayScrollbar className="min-h-0 flex-1">
          <main className="p-6">
            <TabsContent value="model">
              <section className="space-y-4">
                <BayesIssueBanner error={inference.error} validation={null} />
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
              <BayesIssueBanner error={inference.error} validation={validation.report} />
              <ResultOverview result={inference.result} />
            </TabsContent>
          </main>
        </OverlayScrollbar>
      </Tabs>
    </div>
  );
}

function bayesColumnDType(type: string): 'number' | 'integer' | 'boolean' | 'string' | 'date' | 'unknown' {
  const normalized = type.toLowerCase();
  if (normalized.includes('int')) return 'integer';
  if (normalized.includes('float') || normalized.includes('double') || normalized.includes('real') || normalized.includes('decimal') || normalized.includes('numeric')) return 'number';
  if (normalized.includes('bool')) return 'boolean';
  if (normalized.includes('date') || normalized.includes('time')) return 'date';
  if (normalized.includes('char') || normalized.includes('text') || normalized.includes('string')) return 'string';
  return 'unknown';
}

function sameBayesDataset(left: BayesDatasetSelectionDTO, right: BayesDatasetSelectionDTO | null): boolean {
  if (!right) return false;
  if (left.sourceId !== right.sourceId || left.sourceType !== right.sourceType) return false;
  if (left.columns.length !== right.columns.length) return false;
  return left.columns.every((column, index) => {
    const other = right.columns[index];
    return other && column.name === other.name && column.dtype === other.dtype && column.nullable === other.nullable;
  });
}

function BayesIssueBanner({
  error,
  validation,
}: {
  error: BayesInferenceError | null;
  validation: ValidationReportDTO | null;
}) {
  const { t } = useTranslation();
  const issues = validation ? [...validation.errors, ...validation.warnings].slice(0, 4) : [];
  if (!error && issues.length === 0) return null;

  return (
    <div className="space-y-2 rounded-md border border-border bg-muted/20 p-3 text-sm">
      {error ? (
        <p className="text-destructive">
          <span className="font-mono">[{error.code}]</span> {error.message}
          {error.detail ? ` (${error.detail})` : ''}
          {error.column ? ` ${t('bayes.issue.column', { column: error.column })}` : ''}
          {typeof error.row === 'number' ? ` ${t('bayes.issue.row', { row: error.row + 1 })}` : ''}
        </p>
      ) : null}

      {issues.map(issue => (
        <p key={`${issue.code}-${issue.path ?? ''}`} className={issue.severity === 'error' ? 'text-destructive' : 'text-muted-foreground'}>
          <span className="font-mono">[{issue.code}]</span> {issue.message}{issue.path ? ` (${issue.path})` : ''}
        </p>
      ))}
    </div>
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
  phase: ReturnType<typeof useBayesInferenceTask>['phase'];
  task: BayesInferenceTaskDTO | null;
  onRun: () => void | Promise<unknown>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const taskStatus = task?.status ?? null;
  const cancellable = taskStatus === 'queued' || taskStatus === 'running' || taskStatus === 'cancelling';
  const busy = cancellable || phase === 'submitting' || phase === 'reading_result';
  const stageOverride = phase === 'reading_result' ? 'rendering_result' : undefined;
  return (
    <div className="flex shrink-0 items-center gap-3">
      {busy && task ? <BayesProgressStatus task={task} stageOverride={stageOverride} /> : null}
      <Button size="sm" onClick={onRun} disabled={busy || validationLoading}>
        {busy ? t('bayes.actions.running') : t('bayes.actions.run')}
      </Button>
      <Button size="sm" variant="outline" onClick={onCancel} disabled={!cancellable}>
        {t('bayes.actions.cancel')}
      </Button>
    </div>
  );
}



export default BayesView;

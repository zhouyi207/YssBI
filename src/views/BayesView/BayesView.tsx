import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useBayesInferenceTask, useBayesModelDraft, useBayesValidation } from '@/features/application/bayes';
import { usePersistedWindow, useWindowMaximized } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowMenuBar } from '@/shared/ui/WindowChrome';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  FormulaStep,
  ResultOverview,
  SamplerStep,
  SymbolRoleStep,
} from './components/BayesPanels';

export function BayesView() {
  const { t } = useTranslation();
  const isMaximized = useWindowMaximized('BayesView');

  usePersistedWindow('bayes');

  useEffect(() => {
    void getCurrentWindow().show().catch((error) => logger.app.error(String(error), 'BayesView'));
  }, []);

  const databases = useDatabaseStore(state => state.databases);
  const datasets = Object.values(databases).map(database => ({
    sourceType: 'table' as const,
    sourceId: database.id,
    columns: (database.columns ?? []).map(column => ({
      name: column.name,
      dtype: bayesColumnDType(column.type),
      nullable: true,
    })),
  }));
  const modelDraft = useBayesModelDraft();
  const validation = useBayesValidation(modelDraft.draft, modelDraft.draftHash);
  const inference = useBayesInferenceTask();
  const canRun = validation.report?.ok === true && !validation.stale;

  const run = async () => {
    let report = validation.report;
    if (!report || validation.stale) {
      report = await validation.validate();
    }
    if (!report.ok) return;
    await inference.run(modelDraft.draft);
  };

  return (
    <div className="flex h-screen min-h-0 flex-col bg-background text-foreground" data-yssbi-workbench>
      <WindowMenuBar windowActions={<WindowChromeControls isMaximized={isMaximized} />}>
        <div className="flex items-center gap-2 px-4 pointer-events-none self-center">
          <div className="w-5 h-5 bg-[var(--accent-color)] rounded flex items-center justify-center">
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
            <TabsTrigger value="model">Model</TabsTrigger>
            <TabsTrigger value="results">Results</TabsTrigger>
          </TabsList>
          <BayesActionBar
            validationOk={validation.report?.ok === true && !validation.stale}
            validationStale={validation.stale}
            validationLoading={validation.loading}
            taskStatus={inference.task?.status ?? null}
            canRun={canRun}
            onValidate={validation.validate}
            onRun={run}
            onCancel={inference.cancel}
          />
        </section>
        <OverlayScrollbar className="min-h-0 flex-1">
          <main className="p-6">
            <TabsContent value="model">
              <section className="space-y-4">
                <FormulaStep draft={modelDraft.draft} onModelEquationChange={modelDraft.updateModelEquation} />
                <SymbolRoleStep
                  draft={modelDraft.draft}
                  datasets={datasets}
                  onDatasetChange={modelDraft.updateDataset}
                  onSymbolNameChange={modelDraft.updateSymbolName}
                  onSymbolRoleChange={modelDraft.updateSymbolRole}
                  onSymbolDataBindingChange={modelDraft.updateSymbolDataBinding}
                  onSymbolPriorChange={modelDraft.updateSymbolPrior}
                  onSymbolConstraintChange={modelDraft.updateSymbolConstraint}
                  onDeleteSymbol={modelDraft.deleteSymbol}
                />
                <SamplerStep draft={modelDraft.draft} onSamplerChange={modelDraft.updateSampler} />
              </section>
            </TabsContent>
            <TabsContent value="results">
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

function BayesActionBar({
  validationOk,
  validationStale,
  validationLoading,
  taskStatus,
  canRun,
  onValidate,
  onRun,
  onCancel,
}: {
  validationOk: boolean;
  validationStale: boolean;
  validationLoading: boolean;
  taskStatus: string | null;
  canRun: boolean;
  onValidate: () => void | Promise<unknown>;
  onRun: () => void | Promise<unknown>;
  onCancel: () => void;
}) {
  const running = taskStatus === 'running';
  return (
    <div className="flex shrink-0 items-center gap-2">
      <Badge variant={validationOk ? 'default' : validationStale ? 'warning' : 'secondary'}>
        {validationLoading ? 'validating' : validationOk ? 'valid' : validationStale ? 'stale' : 'not validated'}
      </Badge>
      {taskStatus && <Badge variant={running ? 'warning' : 'secondary'}>{taskStatus}</Badge>}
      <Button size="sm" variant="outline" onClick={onValidate} disabled={validationLoading || running}>
        {validationLoading ? 'Validating...' : 'Validate'}
      </Button>
      <Button size="sm" onClick={onRun} disabled={running || (!canRun && validationLoading)}>
        {running ? 'Running...' : 'Run'}
      </Button>
      <Button size="sm" variant="outline" onClick={onCancel} disabled={!running}>
        Cancel
      </Button>
    </div>
  );
}

export default BayesView;

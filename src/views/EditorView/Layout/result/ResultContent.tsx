import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { VscPreview } from 'react-icons/vsc';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { launchInspectablePresentation } from '@/features/application/execution/openInspectableResult';
import {
  loadPresentationWindow,
  parsePlotPayload,
  presentationWindowErrorMessage,
  type PresentationWindowState,
} from '@/features/application/presentation';
import { UnifiedResultView, ResultViewPresentationProvider } from '@/features/application/results';
import type { ResultDescriptor } from '@/shared/types/domain/result';
import { ReportView } from '@/views/InfoView/ReportView';
import { PlotResultView } from '@/features/application/presentation/PlotResultView';

function ResultStatus({ message }: { message: string }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-background p-4 text-center text-muted-foreground">
      <VscPreview className="size-10 opacity-30" aria-hidden />
      <span className="text-xs">{message}</span>
    </div>
  );
}

export function ResultContent({ resultId }: { resultId: string }) {
  const { t } = useTranslation();
  const [reload, setReload] = useState(0);
  const [state, setState] = useState<PresentationWindowState>({ status: 'loading' });
  const [expandFailed, setExpandFailed] = useState(false);
  const expandRequestGeneration = useRef(0);

  useEffect(() => {
    let cancelled = false;
    expandRequestGeneration.current += 1;
    setExpandFailed(false);
    setState({ status: 'loading' });
    void loadPresentationWindow(resultId).then((next) => {
      if (!cancelled) setState(next);
    });
    return () => {
      cancelled = true;
      expandRequestGeneration.current += 1;
    };
  }, [resultId, reload]);

  const expandPlot = async (descriptor: ResultDescriptor) => {
    const requestGeneration = ++expandRequestGeneration.current;
    setExpandFailed(false);
    try {
      await launchInspectablePresentation(descriptor, t('detail.result.expandPlot'));
    } catch {
      if (expandRequestGeneration.current === requestGeneration) {
        setExpandFailed(true);
      }
    }
  };

  const error = presentationWindowErrorMessage(state, {
    missingResultId: t('sourceInspector.missingResultId'),
    notFound: t('sourceInspector.noSource'),
    loadFailed: t('sourceInspector.loadFailed'),
    pending: (completed, total) => t('resultState.pending', { completed, total: total ?? '?' }),
    executionFailed: t('resultState.executionFailed'),
    upstreamFailed: t('resultState.upstreamFailed'),
    cancelled: t('resultState.cancelled'),
  });

  if (state.status === 'loading') return <ResultStatus message={t('common.loading')} />;
  if (state.status === 'pending') {
    return (
      <ResultStatus
        message={t('resultState.pending', {
          completed: state.progress.completed,
          total: state.progress.total ?? '?',
        })}
      />
    );
  }
  if (error) {
    return (
      <Alert className="m-3 w-auto">
        <AlertTitle>{t('detail.result.readFailedTitle')}</AlertTitle>
        <AlertDescription className="flex flex-col gap-2">
          <p>{error}</p>
          {state.status === 'load_failed' ? (
            <Button size="sm" variant="outline" onClick={() => setReload((value) => value + 1)}>
              {t('common.retry')}
            </Button>
          ) : null}
        </AlertDescription>
      </Alert>
    );
  }
  if (state.status !== 'ready') return null;
  if (state.payload.mode === 'inspector') {
    return (
      <ResultViewPresentationProvider presentation="embedded">
        <UnifiedResultView
          payload={state.payload.descriptor}
          renderInfo={(descriptor) => descriptor.presentation.kind === 'report' ? (
            <ReportView
              descriptor={descriptor}
              report={descriptor.presentation.report}
            />
          ) : null}
        />
      </ResultViewPresentationProvider>
    );
  }
  if (state.payload.mode === 'report') {
    if (state.descriptor.presentation.kind !== 'report') return null;
    return (
      <ResultViewPresentationProvider presentation="embedded">
        <ReportView
          descriptor={state.descriptor}
          report={state.descriptor.presentation.report}
          data={state.payload.data}
        />
      </ResultViewPresentationProvider>
    );
  }
  if (state.payload.mode === 'plot') {
    const plotPayload = parsePlotPayload(state.payload.chart, state.payload.data);
    return (
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden bg-background">
        <div className="flex h-(--panel-toolbar-height) shrink-0 items-center justify-end border-b border-border/20 bg-background px-2">
          <Button
            type="button"
            size="xs"
            variant="outline"
            onClick={() => void expandPlot(state.descriptor)}
          >
            {t('detail.result.expandPlot')}
          </Button>
        </div>
        {expandFailed ? (
          <Alert variant="destructive" className="m-3 w-auto" role="alert">
            <AlertTitle>{t('detail.result.expandFailedTitle')}</AlertTitle>
            <AlertDescription>{t('detail.result.expandFailed')}</AlertDescription>
          </Alert>
        ) : null}
        <div className="flex min-h-0 flex-1 flex-col p-3">
          <PlotResultView
            payload={plotPayload}
            invalidContent={t('detail.result.invalidPlot')}
          />
        </div>
      </div>
    );
  }
  return null;
}

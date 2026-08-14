import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { VscPreview } from 'react-icons/vsc';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import {
  loadPresentationWindow,
  presentationWindowErrorMessage,
  type PresentationWindowState,
} from '@/features/application/presentation';
import { useEditorStore } from '@/features/core/editor';
import { ReportResultView, UnifiedResultView } from '@/features/core/resultSource';
import { detailSectionTitleClass } from './shared/detailStyles';
import { workbenchPanelHeaderClass } from '../workbenchPanelHeaderStyles';

function InspectorMessage({ message }: { message: string }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 p-4 text-center text-muted-foreground">
      <VscPreview className="size-10 opacity-30" aria-hidden />
      <span className="text-xs">{message}</span>
    </div>
  );
}

export function InspectorPane() {
  const { t } = useTranslation();
  const resultId = useEditorStore((state) => state.inspectorResultId);
  const [state, setState] = useState<PresentationWindowState | null>(null);

  useEffect(() => {
    if (!resultId) {
      setState(null);
      return;
    }

    let cancelled = false;
    setState({ status: 'loading' });
    void loadPresentationWindow(resultId).then((next) => {
      if (!cancelled) setState(next);
    });
    return () => {
      cancelled = true;
    };
  }, [resultId]);

  if (!resultId) {
    return (
      <div className="flex h-full min-h-0 flex-col bg-background/40">
        <div className={workbenchPanelHeaderClass}>
          <span className={detailSectionTitleClass}>{t('detail.inspector.title')}</span>
        </div>
        <Empty className="min-h-0 rounded-none p-4">
          <EmptyHeader>
            <EmptyMedia variant="icon" className="size-10 text-muted-foreground">
              <VscPreview className="size-5" />
            </EmptyMedia>
            <EmptyTitle>{t('detail.inspector.emptyTitle')}</EmptyTitle>
            <EmptyDescription>{t('detail.inspector.empty')}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </div>
    );
  }

  if (!state || state.status === 'loading') {
    return (
      <div className="flex h-full min-h-0 flex-col bg-background/40">
        <div className={workbenchPanelHeaderClass}>
          <span className={detailSectionTitleClass}>{t('detail.inspector.title')}</span>
        </div>
        <InspectorMessage message={t('common.loading')} />
      </div>
    );
  }

  const error = presentationWindowErrorMessage(state, {
    missingResultId: t('sourceInspector.missingResultId'),
    notFound: t('sourceInspector.noSource'),
    loadFailed: t('sourceInspector.loadFailed'),
  });
  const title = 'descriptor' in state
    ? state.descriptor.title || t('detail.inspector.title')
    : t('detail.inspector.title');

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background/40">
      <div className={workbenchPanelHeaderClass}>
        <span className={`${detailSectionTitleClass} min-w-0 truncate`}>{title}</span>
      </div>
      {error ? <InspectorMessage message={error} /> : null}
      {state.status === 'ready' && state.payload.mode === 'inspector' ? (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <UnifiedResultView payload={state.payload.descriptor} />
        </div>
      ) : null}
      {state.status === 'ready' && state.payload.mode === 'report' ? (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <ReportResultView payload={state.descriptor} data={state.payload.data} />
        </div>
      ) : null}
      {state.status === 'ready' && state.payload.mode === 'plot' ? (
        <InspectorMessage message={t('detail.inspector.plotWindowOnly')} />
      ) : null}
    </div>
  );
}

import { useTranslation } from 'react-i18next';
import { VscPreview } from 'react-icons/vsc';
import { usePresentationWindow } from '@/features/application/presentation';
import { UnifiedResultView } from '@/features/application/viewCapabilities';
import { PresentationWindowShell } from '@/features/application/window/PresentationWindowShell';
import { ReportView } from '@/views/InfoView/ReportView';

export const SourceInspectorWindow: React.FC = () => {
  const { t } = useTranslation();
  const { state, windowActions } = usePresentationWindow('sourceInspector');

  const title =
    state.status === 'ready'
      ? state.descriptor.title
      : t('sourceInspector.title');

  return (
    <PresentationWindowShell
      title={title}
      icon={
        <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscPreview size={14} />
        </span>
      }
      state={state}
      windowActions={windowActions}
      errorMessages={{
        missingResultId: t('sourceInspector.missingResultId'),
        notFound: t('sourceInspector.noSource'),
        loadFailed: t('sourceInspector.loadFailed'),
      }}
    >
      {state.status === 'ready' && state.payload.mode === 'inspector' ? (
        <UnifiedResultView
          payload={state.payload.descriptor}
          renderInfo={(descriptor) => descriptor.presentation.kind === 'report' ? (
            <ReportView
              descriptor={descriptor}
              report={descriptor.presentation.report}
            />
          ) : null}
        />
      ) : null}
      {state.status === 'ready' &&
      state.payload.mode === 'report' &&
      state.descriptor.presentation.kind === 'report' ? (
        <ReportView
          descriptor={state.descriptor}
          report={state.descriptor.presentation.report}
          data={state.payload.data}
        />
      ) : null}
    </PresentationWindowShell>
  );
};

import { useTranslation } from 'react-i18next';
import { VscPreview } from 'react-icons/vsc';
import { usePresentationWindow } from '@/features/application/presentation';
import { UnifiedSourceView } from '@/features/core/resultSource';
import { PresentationWindowShell } from '@/features/application/window/PresentationWindowShell';

export const SourceInspectorWindow: React.FC = () => {
  const { t } = useTranslation();
  const { state, isMaximized } = usePresentationWindow('sourceInspector', 'SourceInspectorWindow');

  const title =
    state.status === 'ready'
      ? state.descriptor.title
      : t('sourceInspector.title');

  return (
    <PresentationWindowShell
      logTag="SourceInspectorWindow"
      title={title}
      icon={
        <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscPreview size={14} />
        </span>
      }
      state={state}
      isMaximized={isMaximized}
      errorMessages={{
        missingSourceId: t('sourceInspector.missingSourceId'),
        notFound: t('sourceInspector.noSource'),
        loadFailed: t('sourceInspector.loadFailed'),
      }}
    >
      {state.status === 'ready' && state.payload.mode === 'inspector' ? (
        <UnifiedSourceView payload={state.payload.descriptor} layout="window" />
      ) : null}
    </PresentationWindowShell>
  );
};

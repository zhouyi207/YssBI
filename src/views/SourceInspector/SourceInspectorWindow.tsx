import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscPreview } from 'react-icons/vsc';
import { SourceService } from '@/services/resultSource/resultSourceService';
import { UnifiedSourceView, type SourceDescriptor } from '@/features/core/resultSource';
import { usePersistedWindow, useReleaseResultSourceOnUnmount, useWindowMaximized } from '@/features/application/window';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';

function getSourceIdFromUrl(): string | null {
  const searchValue = new URLSearchParams(window.location.search).get('sourceId');
  if (searchValue) return searchValue;

  const hashQueryIndex = window.location.hash.indexOf('?');
  if (hashQueryIndex < 0) return null;
  return new URLSearchParams(window.location.hash.slice(hashQueryIndex + 1)).get('sourceId');
}

export const SourceInspectorWindow: React.FC = () => {
  const { t } = useTranslation();
  const sourceId = useMemo(() => getSourceIdFromUrl(), []);
  useReleaseResultSourceOnUnmount(sourceId);
  const [sourceDescriptor, setSourceDescriptor] = useState<SourceDescriptor | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const isMaximized = useWindowMaximized('SourceInspectorWindow');

  usePersistedWindow('sourceInspector');

  useEffect(() => {
    let cancelled = false;

    const revealWindow = async (title?: string) => {
      const windowTitle = title ?? t('sourceInspector.title');
      await getCurrentWindow().setTitle(windowTitle).catch(() => {});
      await getCurrentWindow().show().catch(() => {});
    };

    if (!sourceId) {
      setSourceError(t('sourceInspector.missingSourceId'));
      void revealWindow();
      return;
    }

    (async () => {
      try {
        const descriptor = await SourceService.getDescriptor(sourceId);
        if (cancelled) return;
        if (!descriptor) {
          setSourceError(t('sourceInspector.noSource'));
          await revealWindow();
          return;
        }
        setSourceDescriptor(descriptor);
        await revealWindow(descriptor.title || t('sourceInspector.title'));
      } catch (e) {
        if (!cancelled) {
          setSourceError(e instanceof Error ? e.message : String(e));
          await revealWindow();
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [sourceId, t]);

  const chromeTitle = sourceDescriptor?.title ?? t('sourceInspector.title');

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-[var(--workbench-bg)] font-sans text-[var(--workbench-fg)]">
      <WindowTitleBar childWindow>
        <div className="flex min-w-0 flex-1 items-center gap-2 px-3" data-tauri-drag-region>
          <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
            <VscPreview size={14} />
          </span>
          <span className="min-w-0 truncate text-[13px] font-semibold tracking-tight text-foreground">
            {chromeTitle}
          </span>
        </div>
        <WindowTitleBarActions>
          <WindowChromeControls isMaximized={isMaximized} />
        </WindowTitleBarActions>
      </WindowTitleBar>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {sourceError ? (
          <div className="flex flex-1 items-center justify-center text-sm text-destructive">
            {sourceError}
          </div>
        ) : sourceDescriptor ? (
          <UnifiedSourceView payload={sourceDescriptor} layout="window" />
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            {t('sourceInspector.loading')}
          </div>
        )}
      </div>
    </div>
  );
};

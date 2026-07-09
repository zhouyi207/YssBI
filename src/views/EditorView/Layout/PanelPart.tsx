import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  DEFAULT_PANEL_VIEWS,
  resolvePanelViewComponent,
  type PanelViewDescriptor,
  type PanelViewId,
} from '@/features/core/layout/panelPartModel';
import { setPanelActiveView } from '@/features/core/layout/workbenchLayoutService';
import { viewRegistry } from '../Renderer/viewRegistry';
import { cn } from '@/lib/utils';

export function PanelPart() {
  const { t } = useTranslation();
  const { views, activeViewId } = useLayoutStore(useShallow((s) => {
    const panel = s.nodes.panel;
    return {
      views: (panel?.data?.panelViews ?? DEFAULT_PANEL_VIEWS) as PanelViewDescriptor[],
      activeViewId: (panel?.data?.activePanelView ?? 'logs') as PanelViewId,
    };
  }));

  const ActiveComponent = useMemo(
    () => viewRegistry.get(resolvePanelViewComponent(views, activeViewId)),
    [views, activeViewId],
  );

  const labels: Record<PanelViewId, string> = {
    logs: t('panel.logs'),
    output: t('panel.output'),
  };

  return (
    <div className="flex h-full w-full flex-col overflow-hidden bg-[var(--workbench-bg)]">
      <div className="flex h-[var(--titlebar-height)] shrink-0 items-end gap-0 border-b border-border/40 px-1">
        {views.map((view) => {
          const active = view.id === activeViewId;
          return (
            <button
              key={view.id}
              type="button"
              onClick={() => setPanelActiveView(view.id as PanelViewId)}
              className={cn(
                'relative px-3 pb-1.5 pt-1 text-[11px] font-medium uppercase tracking-wide transition-colors',
                active
                  ? 'text-foreground after:absolute after:inset-x-1 after:bottom-0 after:h-0.5 after:bg-[var(--accent-color)]'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              {labels[view.id as PanelViewId] ?? view.id}
            </button>
          );
        })}
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {ActiveComponent ? <ActiveComponent /> : null}
      </div>
    </div>
  );
}

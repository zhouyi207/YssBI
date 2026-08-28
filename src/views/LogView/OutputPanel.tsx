import { useTranslation } from 'react-i18next';
import { FiTrash2 } from 'react-icons/fi';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useExecutionStore, useGraphSessionStore } from '@/features/application/viewCapabilities';
import type { RunOutputProjection } from '@/shared/types/ui';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';

const EMPTY_RUN_OUTPUT: RunOutputProjection = {
  runId: null,
  entries: [],
  projectionDropped: false,
};

export function OutputPanel() {
  const { t } = useTranslation();
  const graphPath = useGraphSessionStore(
    (state) => state.focusedSession?.graphPath ?? null,
  );
  const output = useExecutionStore((state) => (
    graphPath ? state.graphs[graphPath]?.runOutput ?? EMPTY_RUN_OUTPUT : EMPTY_RUN_OUTPUT
  ));
  const clearRunOutput = useExecutionStore((state) => state.clearRunOutput);
  const hasOutput = output.entries.length > 0 || output.projectionDropped;

  return (
    <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
      <div
        data-output-panel-header
        className="flex h-(--logs-tab-height) shrink-0 items-center justify-between gap-1 border-b border-border/20 bg-background px-1"
      >
        <span className="min-w-0 truncate px-1 text-xs font-medium text-foreground">
          {t('panel.output')}
        </span>
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          disabled={!graphPath || !hasOutput}
          onClick={() => {
            if (graphPath) clearRunOutput(graphPath);
          }}
          tooltip={t('panel.outputClear')}
          aria-label={t('panel.outputClear')}
        >
          <FiTrash2 />
        </ToolbarIconButton>
      </div>

      {!graphPath ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t('panel.outputNoGraph')}
        </div>
      ) : !hasOutput ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t('panel.outputEmpty')}
        </div>
      ) : (
        <ScrollArea orientation="both" className="min-h-0 flex-1">
          <div className="min-w-max py-1 font-mono text-xs" role="log" aria-live="polite">
            {output.projectionDropped ? (
              <div className="px-3 py-1.5 text-amber-500">
                {t('panel.outputProjectionDropped')}
              </div>
            ) : null}
            {output.entries.map((entry) => (
              <div
                key={`${entry.runId}:${entry.sequence}`}
                className="grid grid-cols-[4rem_4rem_minmax(10rem,1fr)] items-start gap-2 border-b border-border/10 px-3 py-1.5 last:border-b-0"
              >
                <span className="text-right text-muted-foreground">{entry.sequence}</span>
                <span className={entry.stream === 'stderr' ? 'text-destructive' : 'text-primary'}>
                  {entry.stream}
                </span>
                <div className="min-w-0">
                  {'text' in entry ? (
                    <pre className="whitespace-pre-wrap wrap-break-word text-foreground">{entry.text}</pre>
                  ) : (
                    <span className="text-amber-500">
                      {t(entry.status === 'truncated'
                        ? 'panel.outputTruncated'
                        : 'panel.outputDropped')}
                    </span>
                  )}
                  <div
                    className="truncate text-[10px] text-muted-foreground/70"
                    title={`${entry.sourceGraphPath} · ${entry.sourceNodeId}`}
                  >
                    {t('panel.outputSource')}: {entry.sourceGraphPath} · {entry.sourceNodeId}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </ScrollArea>
      )}
    </div>
  );
}

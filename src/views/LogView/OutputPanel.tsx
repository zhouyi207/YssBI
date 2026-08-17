import { useTranslation } from 'react-i18next';
import { FiTrash2 } from 'react-icons/fi';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useExecutionStore } from '@/features/core/execution';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
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

  if (!graphPath) {
    return (
      <div className="flex h-full items-center justify-center px-4 text-xs text-muted-foreground">
        {t('panel.outputNoGraph')}
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--workbench-bg)] text-[var(--workbench-fg)]">
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border/40 px-2">
        <span className="min-w-0 truncate font-mono text-[10px] text-muted-foreground">
          {graphPath}
        </span>
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          disabled={!hasOutput}
          onClick={() => clearRunOutput(graphPath)}
          tooltip={t('panel.outputClear')}
          aria-label={t('panel.outputClear')}
        >
          <FiTrash2 size={14} />
        </ToolbarIconButton>
      </div>

      {!hasOutput ? (
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
                className="grid grid-cols-[4rem_4rem_minmax(10rem,1fr)] items-start gap-2 border-b border-border/20 px-3 py-1.5 last:border-b-0"
              >
                <span className="text-right text-muted-foreground">{entry.sequence}</span>
                <span className={entry.stream === 'stderr' ? 'text-red-400' : 'text-blue-400'}>
                  {entry.stream}
                </span>
                <div className="min-w-0">
                  {'text' in entry ? (
                    <pre className="whitespace-pre-wrap break-words text-foreground">{entry.text}</pre>
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

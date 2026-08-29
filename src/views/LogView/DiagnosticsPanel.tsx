import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscError, VscInfo, VscWarning } from 'react-icons/vsc';
import { ScrollArea } from '@/components/ui/scroll-area';
import { revealDiagnosticNode } from '@/features/application/editor/rightSidebarActions';
import { useGraphRead } from '@/features/core/graph/read';
import { useGraphSessionUi } from '@/features/core/graphSession/ui';
import type { FocusedGraphSession } from '@/features/core/graphSession/graphSessionStore';
import { collectNodeDiagnostics } from '@/features/domain/graphDiagnostics/nodeDiagnostics';
import type { GraphNodeDiagnostic } from '@/features/domain/graphDiagnostics/nodeDiagnostics';

function severityIcon(severity: GraphNodeDiagnostic['diagnostic']['severity']) {
  if (severity === 'error') return VscError;
  if (severity === 'warning') return VscWarning;
  return VscInfo;
}

function severityClass(severity: GraphNodeDiagnostic['diagnostic']['severity']): string {
  if (severity === 'error') return 'text-destructive';
  if (severity === 'warning') return 'text-amber-500';
  return 'text-muted-foreground';
}

function activateDiagnostic(
  row: GraphNodeDiagnostic,
  focused: FocusedGraphSession | null,
): void {
  if (!focused || focused.graphPath !== row.graphPath) return;

  void revealDiagnosticNode(row.graphPath, row.nodeId, focused.groupId);
}

export function DiagnosticsPanel() {
  const { t } = useTranslation();
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  const graphPath = focusedSession?.graphPath ?? null;
  const bucket = useGraphRead((snapshot) => (
    graphPath ? snapshot.graphEntities[graphPath] : undefined
  ));
  const rows = useMemo(
    () => collectNodeDiagnostics(graphPath ?? '', bucket),
    [bucket, graphPath],
  );

  return (
    <div
      data-diagnostics-panel
      className="flex h-full min-h-0 flex-col bg-background text-foreground"
    >
      <div
        data-diagnostics-panel-header
        className="flex h-(--logs-tab-height) shrink-0 items-center justify-between gap-1 border-b border-border/20 bg-background px-1"
      >
        <span className="min-w-0 truncate px-1 text-xs font-medium text-foreground">
          {t('panel.diagnostics')}
        </span>
        <span className="shrink-0 px-1 text-[11px] text-muted-foreground">
          {t('panel.diagnosticsCount', { count: rows.length })}
        </span>
      </div>

      {!graphPath ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t('panel.diagnosticsNoGraph')}
        </div>
      ) : rows.length === 0 ? (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-xs text-muted-foreground">
          {t('panel.diagnosticsEmpty')}
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <div className="py-1" role="list" aria-label={t('panel.diagnostics')}>
            {rows.map((row, index) => {
              const Icon = severityIcon(row.diagnostic.severity);
              const iconClass = severityClass(row.diagnostic.severity);
              return (
                <button
                  key={`${row.nodeId}:${row.diagnostic.code}:${index}`}
                  type="button"
                  data-diagnostics-row
                  className="flex w-full items-start gap-2 border-b border-border/10 px-3 py-2 text-left transition-colors hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
                  onClick={() => activateDiagnostic(row, focusedSession)}
                  title={t('panel.diagnosticsLocateNode')}
                >
                  <Icon className={`mt-0.5 size-3.5 shrink-0 ${iconClass}`} aria-hidden />
                  <span className="min-w-0 flex-1">
                    <span className="flex min-w-0 items-baseline gap-2">
                      <span className="truncate text-xs font-medium" title={row.nodeTitle}>
                        {row.nodeTitle}
                      </span>
                      <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                        {row.diagnostic.code}
                      </span>
                    </span>
                    <span className="block break-words text-xs text-foreground">
                      {row.diagnostic.message}
                    </span>
                    {row.locationLabel !== row.nodeTitle ? (
                      <span className="block truncate text-[10px] text-muted-foreground/70">
                        {row.locationLabel}
                      </span>
                    ) : null}
                  </span>
                </button>
              );
            })}
          </div>
        </ScrollArea>
      )}
    </div>
  );
}

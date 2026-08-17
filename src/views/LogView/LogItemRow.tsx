import type { MouseEvent } from 'react';
import { LOG_ITEM_HEIGHT } from '@/app/appConfig/default';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import {
  formatDiagnosticTime,
  getLogDomainColor,
  getLogLevelBackground,
  getLogLevelColor,
  LOG_DOMAIN_BACKGROUND,
  LOG_DOMAIN_LABELS,
} from './logPresentation';

export function LogItemRow({
  log,
  isSelected,
  onClick,
}: {
  log: DiagnosticRecordDto;
  isSelected: boolean;
  onClick: () => void;
}) {
  const levelColor = getLogLevelColor(log.level);
  const levelBg = getLogLevelBackground(log.level);
  const domainColor = getLogDomainColor(log.domain);
  const domainBg = LOG_DOMAIN_BACKGROUND[log.domain] ?? 'bg-muted/40';

  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (event.detail === 0) {
      onClick();
      return;
    }

    const row = event.currentTarget;
    const selection = window.getSelection();
    const hasRowSelection = Boolean(
      selection
        && !selection.isCollapsed
        && ((selection.anchorNode && row.contains(selection.anchorNode))
          || (selection.focusNode && row.contains(selection.focusNode))),
    );
    if (!hasRowSelection) onClick();
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className={[
        'group flex w-full cursor-text select-text items-center gap-2.5 border-b border-border/30 px-3 py-1.5 text-left transition-colors',
        isSelected ? 'bg-[var(--accent-color)]/8' : 'hover:bg-muted/30',
      ].join(' ')}
      style={{ minHeight: LOG_ITEM_HEIGHT }}
    >
      <span className="w-[52px] shrink-0 font-mono text-[10px] tabular-nums text-muted-foreground/80">
        {formatDiagnosticTime(log.timestamp)}
      </span>
      <span
        className={`w-12 shrink-0 rounded px-1 py-0.5 text-center text-[9px] font-semibold uppercase tracking-wide ${levelBg} ${levelColor}`}
      >
        {log.level}
      </span>
      <span
        className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-medium ${domainBg} ${domainColor}`}
      >
        {LOG_DOMAIN_LABELS[log.domain] ?? log.domain.toUpperCase()}
      </span>
      {log.source ? (
        <span className="max-w-[88px] shrink-0 truncate font-mono text-[10px] text-sky-500/80">
          [{log.source}]
        </span>
      ) : null}
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] leading-5 text-foreground/90 group-hover:text-foreground">
        {log.message}
      </span>
    </button>
  );
}

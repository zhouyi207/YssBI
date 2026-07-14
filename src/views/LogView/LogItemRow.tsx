import type { LogLevel, LogMessage } from '@/shared/types/ui';
import { LOG_ITEM_HEIGHT } from '@/app/appConfig/default';
import {
  getLogLevelBackground,
  getLogLevelColor,
  getLogTypeColor,
  LOG_TYPE_BACKGROUND,
  LOG_TYPE_LABELS,
} from './logPresentation';

const LEVEL_ACCENT: Record<LogLevel, string> = {
  error: 'border-l-red-500/70',
  warn: 'border-l-amber-500/70',
  info: 'border-l-sky-500/60',
  debug: 'border-l-border',
  trace: 'border-l-border/60',
};

export function LogItemRow({
  log,
  isSelected,
  onClick,
}: {
  log: LogMessage;
  isSelected: boolean;
  onClick: () => void;
}) {
  const levelColor = getLogLevelColor(log.level);
  const levelBg = getLogLevelBackground(log.level);
  const typeColor = getLogTypeColor(log.log_type);
  const typeBg = LOG_TYPE_BACKGROUND[log.log_type] ?? 'bg-muted/40';

  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        'group flex w-full items-center gap-2.5 border-b border-border/30 px-3 py-1.5 text-left transition-colors',
        'border-l-2',
        LEVEL_ACCENT[log.level] ?? 'border-l-border/50',
        isSelected
          ? 'bg-[var(--accent-color)]/8'
          : 'hover:bg-muted/30',
      ].join(' ')}
      style={{ minHeight: LOG_ITEM_HEIGHT }}
    >
      <span className="w-[52px] shrink-0 font-mono text-[10px] tabular-nums text-muted-foreground/80">
        {log.timestamp.split(' ')[1]}
      </span>
      <span
        className={`w-12 shrink-0 rounded px-1 py-0.5 text-center text-[9px] font-semibold uppercase tracking-wide ${levelBg} ${levelColor}`}
      >
        {log.level}
      </span>
      <span
        className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-medium ${typeBg} ${typeColor}`}
      >
        {LOG_TYPE_LABELS[log.log_type] ?? log.log_type.toUpperCase()}
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

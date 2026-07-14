import { cn } from '@/lib/utils';
import { workbenchPanelHeaderClass } from '@/views/EditorView/Layout/workbenchPanelHeaderStyles';
import { useLogPanelContext } from './logPanelContext';
import { LogPanelStatus } from './LogPanelStatus';
import { LogPanelToolbar } from './LogPanelToolbar';
import { LogTypeTabStrip } from './LogTypeTabStrip';

/** Shared log panel header: type tabs + status + toolbar (embedded and standalone). */
export function LogPanelChrome() {
  const { leadingDragProps } = useLogPanelContext();

  return (
    <div className={`${workbenchPanelHeaderClass} gap-2 border-border/40 px-1`}>
      <div
        className={cn(
          'flex min-w-0 flex-1 items-center gap-2',
          leadingDragProps ? 'cursor-grab select-none active:cursor-grabbing' : '',
        )}
        {...leadingDragProps}
      >
        <LogTypeTabStrip />
        <div className="h-3 w-px shrink-0 bg-border/60" aria-hidden />
        <LogPanelStatus />
      </div>
      <LogPanelToolbar />
    </div>
  );
}

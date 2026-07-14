import { cn } from '@/lib/utils';
import { workbenchPanelHeaderClass } from '@/views/EditorView/Layout/workbenchPanelHeaderStyles';
import { useLogPanelContext } from './logPanelContext';
import { LogPanelStatus } from './LogPanelStatus';
import { LogPanelToolbar } from './LogPanelToolbar';
import { LogTypeTabStrip } from './LogTypeTabStrip';

/** Shared log panel header: draggable tabs row + status + toolbar. */
export function LogPanelChrome() {
  const { dragHandleRef, dragHandleProps } = useLogPanelContext();

  return (
    <div className={`${workbenchPanelHeaderClass} gap-2 border-border/40 px-1`}>
      <div
        ref={dragHandleRef}
        className={cn(
          'flex min-w-0 flex-1 items-center gap-2',
          dragHandleProps ? 'cursor-grab select-none active:cursor-grabbing' : '',
        )}
        draggable={dragHandleProps?.draggable}
        onDragStart={dragHandleProps?.onDragStart}
        onDrag={dragHandleProps?.onDrag}
        onDragEnd={dragHandleProps?.onDragEnd}
      >
        <LogTypeTabStrip />
        <div className="h-3 w-px shrink-0 bg-border/60" aria-hidden />
        <LogPanelStatus />
      </div>
      <LogPanelToolbar />
    </div>
  );
}

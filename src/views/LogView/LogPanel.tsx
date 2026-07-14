import { forwardRef } from 'react';
import { LogPanelContent } from './LogPanelContent';

/** Log list body — `LogPanelProvider` is owned by `PanelPart` (embedded) or `LogWindow` (standalone). */
export const LogPanel = forwardRef<HTMLDivElement, Record<string, never>>((_, ref) => (
  <div ref={ref} className="flex h-full w-full flex-col overflow-hidden">
    <LogPanelContent className="h-full" />
  </div>
));

LogPanel.displayName = 'LogPanel';

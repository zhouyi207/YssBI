import { forwardRef } from 'react';
import { LogPanelContent } from './LogPanelContent';

export const LogPanel = forwardRef<HTMLDivElement, Record<string, never>>((_, ref) => {

  return (
    <div ref={ref} className="flex flex-col h-full w-full overflow-hidden">
      <div className="flex-1 min-h-0">
        <LogPanelContent variant="embedded" className="h-full" />
      </div>
    </div>
  );
});

LogPanel.displayName = 'LogPanel';

import { useLogPanelContext } from './logPanelContext';
import { LogPanelChrome } from './LogPanelChrome';
import { LogPanelList } from './LogPanelList';

export interface LogPanelContentProps {
  className?: string;
}

export function LogPanelContent({ className = '' }: LogPanelContentProps) {
  const { dragPreviewPortal } = useLogPanelContext();

  return (
    <div className={`flex h-full flex-col overflow-hidden bg-[var(--workbench-bg)] text-[var(--workbench-fg)] ${className}`}>
      {dragPreviewPortal}
      <LogPanelChrome />
      <LogPanelList />
    </div>
  );
}

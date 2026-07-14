import { LogPanelChrome } from './LogPanelChrome';
import { LogPanelList } from './LogPanelList';

export interface LogPanelContentProps {
  className?: string;
}

export function LogPanelContent({ className = '' }: LogPanelContentProps) {
  return (
    <div className={`flex h-full flex-col overflow-hidden bg-[var(--workbench-bg)] text-[var(--workbench-fg)] ${className}`}>
      <LogPanelChrome />
      <LogPanelList />
    </div>
  );
}

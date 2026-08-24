import type { ReactNode } from 'react';
import { ScrollArea } from '@/components/ui/scroll-area';

interface DetailPanelShellProps {
  children: ReactNode;
}

export function DetailPanelShell({ children }: DetailPanelShellProps) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-background">
      <ScrollArea className="min-h-0 flex-1" orientation="vertical">
        <div className="flex min-w-0 flex-col divide-y divide-border/20">{children}</div>
      </ScrollArea>
    </div>
  );
}

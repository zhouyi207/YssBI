import type { ReactNode } from 'react';
import { ScrollArea } from '@/components/ui/scroll-area';

interface DetailPanelShellProps {
  children: ReactNode;
}

export function DetailPanelShell({ children }: DetailPanelShellProps) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-background/40">
      <ScrollArea className="flex-1" orientation="vertical">
        <div className="space-y-3 p-3 pb-4">{children}</div>
      </ScrollArea>
    </div>
  );
}

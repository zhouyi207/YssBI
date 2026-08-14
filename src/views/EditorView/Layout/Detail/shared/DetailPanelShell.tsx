import type { ReactNode } from 'react';
import { ScrollArea } from '@/components/ui/scroll-area';
import { workbenchPanelHeaderClass } from '../../workbenchPanelHeaderStyles';
import { detailSectionTitleClass } from './detailStyles';

interface DetailPanelShellProps {
  title: string;
  children: ReactNode;
}

export function DetailPanelShell({ title, children }: DetailPanelShellProps) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-background/40">
      <div className={workbenchPanelHeaderClass}>
        <span className={detailSectionTitleClass}>{title}</span>
      </div>
      <ScrollArea className="flex-1" orientation="vertical">
        <div className="space-y-3 p-3 pb-4">{children}</div>
      </ScrollArea>
    </div>
  );
}

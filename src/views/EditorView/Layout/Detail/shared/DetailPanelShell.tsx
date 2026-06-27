import type { ReactNode } from 'react';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { Separator } from '@/components/ui/separator';
import { detailSectionTitleClass } from './detailStyles';

interface DetailPanelShellProps {
  title: string;
  children: ReactNode;
}

export function DetailPanelShell({ title, children }: DetailPanelShellProps) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-background/40">
      <div
        className="flex shrink-0 items-center justify-between bg-background/80 px-3 backdrop-blur-sm"
        style={{ height: 'var(--titlebar-height)' }}
      >
        <span className={detailSectionTitleClass}>{title}</span>
      </div>
      <Separator />
      <OverlayScrollbar className="flex-1" direction="vertical">
        <div className="space-y-3 p-3 pb-4">{children}</div>
      </OverlayScrollbar>
    </div>
  );
}

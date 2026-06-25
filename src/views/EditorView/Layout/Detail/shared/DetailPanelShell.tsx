import type { ReactNode } from 'react';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { detailSectionTitleClass } from './detailStyles';

interface DetailPanelShellProps {
  title: string;
  children: ReactNode;
}

export function DetailPanelShell({ title, children }: DetailPanelShellProps) {
  return (
    <>
      <div
        className="flex shrink-0 items-center justify-between border-b border-border bg-[var(--workbench-bg)]/50 px-3"
        style={{ height: 'var(--titlebar-height)' }}
      >
        <span className={detailSectionTitleClass}>{title}</span>
      </div>
      <OverlayScrollbar className="flex-1 pb-4" direction="vertical">
        {children}
      </OverlayScrollbar>
    </>
  );
}

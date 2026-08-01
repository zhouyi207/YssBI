import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

export function SidebarEmptyState({
  title,
  description,
  action,
  className,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('min-w-0 px-3 py-3 text-xs', className)}>
      <div className="break-words text-foreground/85">{title}</div>
      {description ? (
        <div className="mt-1 break-words leading-relaxed text-muted-foreground">
          {description}
        </div>
      ) : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}

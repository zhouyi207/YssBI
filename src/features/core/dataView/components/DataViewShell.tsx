import type { ReactNode } from 'react';

interface DataViewShellProps {
  title: string;
  meta?: ReactNode;
  toolbar?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function DataViewShell({
  title,
  meta,
  toolbar,
  children,
  className,
}: DataViewShellProps) {
  return (
    <div className={`mx-auto flex w-full max-w-[1200px] flex-col gap-3 p-6 ${className ?? ''}`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-xl font-bold text-foreground">{title}</h1>
          {meta ? <div className="mt-1 text-sm text-muted-foreground">{meta}</div> : null}
        </div>
        {toolbar ? <div className="flex shrink-0 items-center gap-2">{toolbar}</div> : null}
      </div>
      <div className="min-h-0 flex-1">{children}</div>
    </div>
  );
}

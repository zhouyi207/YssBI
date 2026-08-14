import type { ReactNode } from 'react';

interface ResultViewShellProps {
  title: string;
  meta?: ReactNode;
  toolbar?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function ResultViewShell({
  title,
  meta,
  toolbar,
  children,
  className,
}: ResultViewShellProps) {
  return (
    <div
      className={[
        'flex h-full min-h-0 w-full flex-col gap-3 p-4',
        className ?? '',
      ].join(' ')}
    >
      <div className="flex shrink-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-lg font-bold text-foreground">{title}</h1>
          {meta ? <div className="mt-1 text-sm text-muted-foreground">{meta}</div> : null}
        </div>
        {toolbar ? <div className="flex shrink-0 items-center gap-2">{toolbar}</div> : null}
      </div>
      <div className="flex min-h-0 flex-1 flex-col">{children}</div>
    </div>
  );
}

import type { ReactNode } from 'react';

export type SourceViewLayout = 'window' | 'embedded';

interface SourceViewShellProps {
  title: string;
  meta?: ReactNode;
  toolbar?: ReactNode;
  children: ReactNode;
  className?: string;
  layout?: SourceViewLayout;
}

export function SourceViewShell({
  title,
  meta,
  toolbar,
  children,
  className,
  layout = 'embedded',
}: SourceViewShellProps) {
  const isWindow = layout === 'window';

  return (
    <div
      className={[
        'flex w-full flex-col gap-3',
        isWindow ? 'h-full min-h-0 p-4' : 'mx-auto max-w-[1200px] p-6',
        className ?? '',
      ].join(' ')}
    >
      <div className="flex shrink-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h1
            className={[
              'truncate font-bold text-foreground',
              isWindow ? 'text-lg' : 'text-xl',
            ].join(' ')}
          >
            {title}
          </h1>
          {meta ? <div className="mt-1 text-sm text-muted-foreground">{meta}</div> : null}
        </div>
        {toolbar ? <div className="flex shrink-0 items-center gap-2">{toolbar}</div> : null}
      </div>
      <div className={isWindow ? 'flex min-h-0 flex-1 flex-col' : 'min-h-0 flex-1'}>{children}</div>
    </div>
  );
}

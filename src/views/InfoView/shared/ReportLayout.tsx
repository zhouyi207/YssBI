import type { ReactNode } from 'react';
import { Suspense } from 'react';
import { useResultViewPresentation } from '@/features/application/viewCapabilities';
import { SectionHeader } from './RegressionShared';
import { REPORT_SECTION_ICONS, type ReportSectionIcon } from './reportIcons';

const LAZY_FALLBACKS = {
  formula: 'rounded-lg border border-border bg-card h-24 animate-pulse',
  chart: 'rounded-lg border border-border bg-card h-[280px] animate-pulse',
} as const;

const LAYOUT_SIZE_CLASS = {
  default: 'max-w-[900px]',
  wide: 'max-w-[980px]',
  extraWide: 'max-w-[1100px]',
} as const;

export type ReportLazyVariant = keyof typeof LAZY_FALLBACKS;
export type ReportLayoutSize = keyof typeof LAYOUT_SIZE_CLASS;

export function ReportLayout({
  title,
  badges,
  subtitle,
  children,
  size = 'default',
}: {
  title: string;
  badges?: ReactNode;
  subtitle?: ReactNode;
  children: ReactNode;
  size?: ReportLayoutSize;
}) {
  const presentation = useResultViewPresentation();
  const showHeading = presentation === 'standalone';

  return (
    <div className={`mx-auto p-6 ${LAYOUT_SIZE_CLASS[size]}`}>
      {showHeading ? (
        <div className="mb-6">
          <h1 className="mb-2 text-xl font-bold text-foreground">{title}</h1>
          {badges ? <div className="flex flex-wrap items-center gap-3">{badges}</div> : null}
          {subtitle ? <div className="mt-1">{subtitle}</div> : null}
        </div>
      ) : badges || subtitle ? (
        <div className="mb-4">
          {badges ? <div className="flex flex-wrap items-center gap-3">{badges}</div> : null}
          {subtitle ? <div className="mt-1">{subtitle}</div> : null}
        </div>
      ) : null}
      {children}
    </div>
  );
}
export function ReportSection({
  title,
  icon,
  children,
}: {
  title: string;
  icon: ReportSectionIcon;
  children: ReactNode;
}) {
  return (
    <>
      <SectionHeader title={title} icon={REPORT_SECTION_ICONS[icon]} />
      {children}
    </>
  );
}

export function ReportLazyBoundary({
  variant,
  children,
}: {
  variant: ReportLazyVariant;
  children: ReactNode;
}) {
  return <Suspense fallback={<div className={LAZY_FALLBACKS[variant]} />}>{children}</Suspense>;
}

export function ReportSubheading({
  title,
  timingMs,
  trailing,
}: {
  title: string;
  timingMs?: number | null;
  trailing?: ReactNode;
}) {
  return (
    <div className="mb-2 flex items-center justify-between px-1">
      <span className="text-[11px] uppercase tracking-wider text-muted-foreground">{title}</span>
      {trailing ??
        (timingMs != null ? (
          <span className="font-mono text-[10px] text-primary">{timingMs} ms</span>
        ) : null)}
    </div>
  );
}

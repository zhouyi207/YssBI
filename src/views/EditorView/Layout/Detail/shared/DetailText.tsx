import type { ComponentPropsWithoutRef, ElementType, ReactNode } from 'react';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import {
  detailAccentMonoTextClass,
  detailBadgeClass,
  detailBodyTextClass,
  detailMetaTextClass,
  detailMonoTextClass,
  detailSectionTitleClass,
  detailSmallMetaTextClass,
  detailSubsectionTitleClass,
} from './detailStyles';

type DetailTextTone = 'body' | 'muted' | 'smallMuted' | 'mono' | 'accentMono';

const toneClass: Record<DetailTextTone, string> = {
  body: detailBodyTextClass,
  muted: detailMetaTextClass,
  smallMuted: detailSmallMetaTextClass,
  mono: detailMonoTextClass,
  accentMono: detailAccentMonoTextClass,
};

type DetailTextProps<T extends ElementType> = {
  as?: T;
  tone?: DetailTextTone;
  className?: string;
  children: ReactNode;
} & Omit<ComponentPropsWithoutRef<T>, 'as' | 'className' | 'children'>;

export function DetailText<T extends ElementType = 'span'>({
  as,
  tone = 'body',
  className,
  children,
  ...props
}: DetailTextProps<T>) {
  const Component = as ?? 'span';
  return (
    <Component className={cn(toneClass[tone], className)} {...props}>
      {children}
    </Component>
  );
}

interface DetailBadgeProps {
  children: ReactNode;
  className?: string;
  title?: string;
}

export function DetailBadge({ children, className, title }: DetailBadgeProps) {
  return (
    <Badge variant="secondary" className={cn(detailBadgeClass, className)} title={title}>
      {children}
    </Badge>
  );
}

interface DetailSectionHeaderProps {
  children: ReactNode;
  className?: string;
  level?: 'section' | 'subsection';
}

export function DetailSectionHeader({
  children,
  className,
  level = 'section',
}: DetailSectionHeaderProps) {
  return (
    <div
      className={cn(
        'flex items-center',
        level === 'section' ? detailSectionTitleClass : detailSubsectionTitleClass,
        className,
      )}
    >
      {children}
    </div>
  );
}

import type { ReactNode } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { DetailFieldRow } from './DetailFieldRow';
import { DetailText } from './DetailText';
import { detailInlineInputClass, detailTableClass } from './detailStyles';

type DetailLabelWidth = 'narrow' | 'wide' | 'auto';

interface DetailFormProps {
  children: ReactNode;
  className?: string;
}

export function DetailForm({ children, className }: DetailFormProps) {
  return (
    <Card className={cn('rounded-lg bg-card/80 py-0 shadow-xs', detailTableClass, className)}>
      <CardContent className="space-y-2 p-3">{children}</CardContent>
    </Card>
  );
}

interface DetailNameFieldProps {
  label: ReactNode;
  value: string;
  onChange: (value: string) => void;
  labelWidth?: DetailLabelWidth;
}

export function DetailNameField({
  label,
  value,
  onChange,
  labelWidth,
}: DetailNameFieldProps) {
  return (
    <DetailFieldRow label={label} labelWidth={labelWidth}>
      <Input
        className={detailInlineInputClass}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </DetailFieldRow>
  );
}

interface DetailReadonlyFieldProps {
  label: ReactNode;
  children: ReactNode;
  labelWidth?: DetailLabelWidth;
  tone?: 'body' | 'muted' | 'smallMuted' | 'mono' | 'accentMono';
  className?: string;
  labelClassName?: string;
  valueClassName?: string;
}

export function DetailReadonlyField({
  label,
  children,
  labelWidth,
  tone = 'muted',
  className,
  labelClassName,
  valueClassName,
}: DetailReadonlyFieldProps) {
  return (
    <DetailFieldRow
      label={label}
      labelWidth={labelWidth}
      labelClassName={labelClassName}
      valueClassName={valueClassName}
    >
      <DetailText
        as="div"
        tone={tone}
        className={cn(
          'flex min-h-8 items-center rounded-md border border-transparent px-3 py-1',
          className,
        )}
      >
        {children}
      </DetailText>
    </DetailFieldRow>
  );
}

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { DetailFieldRow } from './DetailFieldRow';
import { DetailText } from './DetailText';
import { detailTableClass } from './detailStyles';


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


interface DetailCommitInputProps {
  value: string;
  onCommit: (value: string) => void | Promise<void>;
  className?: string;
  type?: string;
}

export function DetailCommitInput({
  value,
  onCommit,
  className,
  type = 'text',
}: DetailCommitInputProps) {
  const [draft, setDraft] = useState(value);
  const skipNextBlurCommitRef = useRef(false);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const commit = () => {
    if (skipNextBlurCommitRef.current) {
      skipNextBlurCommitRef.current = false;
      return;
    }
    if (draft === value) return;
    void onCommit(draft);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.currentTarget.blur();
      return;
    }
    if (event.key === 'Escape') {
      skipNextBlurCommitRef.current = true;
      setDraft(value);
      event.currentTarget.blur();
    }
  };

  return (
    <Input
      className={className}
      type={type}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={handleKeyDown}
    />
  );
}


interface DetailReadonlyFieldProps {
  label: ReactNode;
  children: ReactNode;
  tone?: 'body' | 'muted' | 'smallMuted' | 'mono' | 'accentMono';
  className?: string;
  labelClassName?: string;
  valueClassName?: string;
}

export function DetailReadonlyField({
  label,
  children,
  tone = 'muted',
  className,
  labelClassName,
  valueClassName,
}: DetailReadonlyFieldProps) {
  return (
    <DetailFieldRow
      label={label}
      labelClassName={labelClassName}
      valueClassName={valueClassName}
    >
      <DetailText
        as="div"
        tone={tone}
        className={cn(
          'flex min-h-8 min-w-0 items-center justify-end truncate rounded-md border border-transparent px-3 py-1 text-right',
          className,
        )}
      >
        {children}
      </DetailText>
    </DetailFieldRow>
  );
}

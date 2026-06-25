import React from 'react';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export function InfoSegmentedToggle<T extends string>({
  value,
  onValueChange,
  options,
  className,
}: {
  value: T;
  onValueChange: (value: T) => void;
  options: { value: T; label: string }[];
  className?: string;
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      onValueChange={(next) => next && onValueChange(next as T)}
      variant="outline"
      size="sm"
      className={cn('text-[11px]', className)}
    >
      {options.map((option) => (
        <ToggleGroupItem key={option.value} value={option.value} className="px-3">
          {option.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}

export function InfoAccentButton({
  children,
  disabled,
  loading,
  onClick,
  className,
}: {
  children: React.ReactNode;
  disabled?: boolean;
  loading?: boolean;
  onClick?: () => void;
  className?: string;
}) {
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      disabled={disabled || loading}
      onClick={onClick}
      className={cn(
        'border-[var(--accent-color)]/40 bg-[var(--accent-color)]/20 text-[var(--accent-color)] hover:bg-[var(--accent-color)]/30',
        className,
      )}
    >
      {loading ? '...' : children}
    </Button>
  );
}

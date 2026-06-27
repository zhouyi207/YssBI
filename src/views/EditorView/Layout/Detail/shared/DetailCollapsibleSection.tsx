import { useState, type ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { DetailText } from './DetailText';

interface DetailCollapsibleSectionProps {
  title: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  contentClassName?: string;
  headerClassName?: string;
}

export function DetailCollapsibleSection({
  title,
  children,
  defaultOpen = false,
  contentClassName,
  headerClassName,
}: DetailCollapsibleSectionProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <Card className="overflow-hidden rounded-lg bg-card/80 py-0 shadow-xs">
      <CardHeader className={cn('px-3 py-2.5', headerClassName)}>
        <Button
          type="button"
          variant="ghost"
          className="h-auto w-full justify-between gap-3 px-1 py-0 text-left hover:bg-transparent aria-expanded:bg-transparent aria-expanded:text-inherit dark:aria-expanded:bg-transparent"
          aria-expanded={isOpen}
          onClick={() => setIsOpen((open) => !open)}
        >
          <DetailText className="min-w-0 truncate font-semibold">{title}</DetailText>
          <svg
            className={`size-3.5 shrink-0 text-muted-foreground transition-transform ${isOpen ? 'rotate-180' : ''}`}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </Button>
      </CardHeader>
      {isOpen && (
        <CardContent className={cn('border-t border-border/60 px-3 pb-3 pt-2', contentClassName)}>
          {children}
        </CardContent>
      )}
    </Card>
  );
}

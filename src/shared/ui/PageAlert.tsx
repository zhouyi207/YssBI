import type { ReactNode } from 'react';
import { VscClose, VscError, VscInfo, VscWarning } from 'react-icons/vsc';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';

export interface PageAlertProps {
  title: ReactNode;
  description?: ReactNode;
  variant?: 'info' | 'warning' | 'destructive';
  actionLabel?: string;
  onAction?: () => void;
  dismissLabel?: string;
  onDismiss?: () => void;
  className?: string;
}

const icons = {
  info: VscInfo,
  warning: VscWarning,
  destructive: VscError,
} as const;

export function PageAlert({
  title,
  description,
  variant = 'info',
  actionLabel,
  onAction,
  dismissLabel,
  onDismiss,
  className,
}: PageAlertProps) {
  const Icon = icons[variant];
  return (
    <Alert variant={variant} className={className}>
      <Icon aria-hidden="true" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription>
        {description ? <p>{description}</p> : null}
        {actionLabel && onAction ? (
          <Button type="button" size="sm" variant="outline" className="mt-2" onClick={onAction}>
            {actionLabel}
          </Button>
        ) : null}
      </AlertDescription>
      {onDismiss ? (
        <Button
          type="button"
          size="icon-sm"
          variant="ghost"
          className="absolute right-1.5 top-1.5"
          aria-label={dismissLabel}
          onClick={onDismiss}
        >
          <VscClose aria-hidden="true" />
        </Button>
      ) : null}
    </Alert>
  );
}

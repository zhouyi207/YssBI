import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  presentationWindowErrorMessage,
  type PresentationWindowState,
} from '@/features/application/presentation';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';
import { logger } from '@/utils/appLogger';

interface PresentationWindowShellProps {
  logTag: string;
  title: string;
  icon?: ReactNode;
  state: PresentationWindowState;
  errorMessages: {
    missingSourceId: string;
    notFound: string;
    loadFailed: string;
  };
  contentClassName?: string;
  isMaximized: boolean;
  children: ReactNode;
}

export function PresentationWindowShell({
  logTag,
  title,
  icon,
  state,
  errorMessages,
  contentClassName = 'flex min-h-0 flex-1 flex-col overflow-hidden',
  isMaximized,
  children,
}: PresentationWindowShellProps) {
  const { t } = useTranslation();
  const error = presentationWindowErrorMessage(state, errorMessages);

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (e) {
      logger.app.error(`Failed to minimize: ${String(e)}`, logTag);
    }
  };

  const handleMaximize = async () => {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (e) {
      logger.app.error(`Failed to maximize: ${String(e)}`, logTag);
    }
  };

  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch (e) {
      logger.app.error(`Failed to close: ${String(e)}`, logTag);
    }
  };

  if (state.status === 'loading') {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-[var(--workbench-bg)] text-muted-foreground">
        {t('common.initializing')}
      </div>
    );
  }

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-[var(--workbench-bg)] text-foreground">
      <WindowTitleBar childWindow>
        <div className="flex min-w-0 flex-1 items-center gap-2 px-4" data-tauri-drag-region>
          {icon}
          <span className="min-w-0 truncate text-sm font-bold tracking-tight text-foreground">
            {title}
          </span>
        </div>
        <WindowTitleBarActions>
          <WindowChromeControls
            isMaximized={isMaximized}
            onMinimize={handleMinimize}
            onMaximize={handleMaximize}
            onClose={handleClose}
          />
        </WindowTitleBarActions>
      </WindowTitleBar>

      <div className={contentClassName}>
        {error ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
            <svg className="h-12 w-12 text-red-500/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
              />
            </svg>
            <span className="text-sm">{error}</span>
          </div>
        ) : (
          children
        )}
      </div>
    </div>
  );
}

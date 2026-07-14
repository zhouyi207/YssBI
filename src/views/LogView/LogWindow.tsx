import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogPanelContent } from './LogPanelContent';
import { LogPanelProvider } from './logPanelContext';
import { usePersistedWindow, useWindowMaximized } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowChrome } from '@/shared/ui/WindowChrome';

export const LogWindow = () => {
  const { t } = useTranslation();
  const isMaximized = useWindowMaximized('LogWindow');

  usePersistedWindow('logs');

  useEffect(() => {
    void getCurrentWindow().show().catch((e) => logger.app.error(String(e), 'LogWindow'));
  }, []);

  const handleMinimize = async () => {
    await getCurrentWindow().minimize();
  };

  const handleMaximize = async () => {
    await getCurrentWindow().toggleMaximize();
  };

  const handleClose = async () => {
    await getCurrentWindow().close();
  };

  return (
    <div className="flex flex-col h-screen bg-[var(--workbench-bg)] text-foreground overflow-hidden">
      {/* 自定义标题栏 - 与主窗口一致 */}
      <WindowChrome
        childWindow
        actions={
          <WindowChromeControls
            isMaximized={isMaximized}
            onMinimize={handleMinimize}
            onMaximize={handleMaximize}
            onClose={handleClose}
          />
        }
      >
        <div className="flex flex-1 items-center gap-2 px-4" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span className="text-foreground font-bold text-sm tracking-tight">{t("log.title")}</span>
        </div>
      </WindowChrome>

      {/* 内容区域 */}
      <div className="min-h-0 flex-1">
        <LogPanelProvider variant="standalone">
          <LogPanelContent className="h-full" />
        </LogPanelProvider>
      </div>
    </div>
  );
};

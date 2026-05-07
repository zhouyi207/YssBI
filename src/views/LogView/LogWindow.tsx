import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogPanelContent } from './LogPanelContent';
import { usePersistedWindow } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import { Button } from '@/components/ui/button';

export const LogWindow = () => {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  usePersistedWindow('logs');

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    let disposed = false;

    const initWindow = async () => {
      const currentWindow = getCurrentWindow();
      await currentWindow.show().catch((e) => logger.app.error(String(e), 'LogWindow'));

      const maximized = await currentWindow.isMaximized();
      if (!disposed) setIsMaximized(maximized);

      const unlisten = await currentWindow.onResized(async () => {
        const maximized = await currentWindow.isMaximized();
        if (!disposed) setIsMaximized(maximized);
      });

      if (disposed) unlisten();
      else cleanup = unlisten;
    };

    initWindow().catch((e) => logger.app.error(String(e), 'LogWindow'));

    return () => {
      disposed = true;
      cleanup?.();
    };
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
    <div className="flex flex-col h-screen bg-[var(--workbench-bg)] text-white overflow-hidden">
      {/* 自定义标题栏 - 与主窗口一致 */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-border flex items-center z-50 select-none shrink-0 rounded-tr-lg overflow-hidden"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span className="text-foreground font-bold text-sm tracking-tight">{t("log.title")}</span>
        </div>

        {/* 窗口控制按钮 */}
        <div className="flex items-center h-full">
          <Button
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={handleMinimize}
            className="h-10 rounded-none text-muted-foreground"
            title={t("common.minimize")}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
            </svg>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={handleMaximize}
            className="h-10 rounded-none text-muted-foreground"
            title={isMaximized ? t('common.restore') : t('common.maximize')}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
            </svg>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={handleClose}
            className="h-10 w-12 rounded-none text-muted-foreground hover:bg-red-600 hover:text-white"
            title={t("common.close")}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </Button>
        </div>
      </div>

      {/* 内容区域 */}
      <div className="flex-1 min-h-0">
        <LogPanelContent variant="standalone" className="h-full" />
      </div>
    </div>
  );
};

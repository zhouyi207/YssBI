import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscDatabase } from 'react-icons/vsc';
import { logger } from '@/utils/appLogger';
import { Button } from '@/components/ui/button';

interface TitleBarProps {
  isModified: boolean;
}

export const TitleBar: React.FC<TitleBarProps> = ({ isModified }) => {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | null = null;
    const setup = async () => {
      const win = getCurrentWindow();
      const maximized = await win.isMaximized();
      if (!disposed) setIsMaximized(maximized);
      const unlisten = await win.onResized(async () => {
        if (!disposed) setIsMaximized(await win.isMaximized());
      });
      if (disposed) unlisten();
      else cleanup = unlisten;
    };
    setup().catch((e) => logger.app.error(String(e), 'DataViewTitleBar'));
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize();
  const handleMaximize = () => getCurrentWindow().toggleMaximize();
  const handleClose = () => getCurrentWindow().close();

  return (
    <div data-tauri-drag-region className="h-9 bg-background/95 border-b border-border flex items-center z-50 select-none shrink-0">
      <div className="flex items-center gap-2 px-3 flex-1" data-tauri-drag-region>
        <span className="flex size-5 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscDatabase size={14} />
        </span>
        <span className="text-foreground font-semibold text-[13px] tracking-tight">{t("dataView.title")}</span>
        {isModified && (
          <span className="rounded-sm bg-yellow-500/10 px-1.5 py-0.5 text-[10px] font-medium text-yellow-600 dark:text-yellow-400">
            {t("dataView.modified")}
          </span>
        )}
      </div>
      <div className="flex items-center h-full">
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMinimize} className="h-9 rounded-none text-muted-foreground" title={t("common.minimize")}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMaximize} className="h-9 rounded-none text-muted-foreground" title={isMaximized ? t("common.restore") : t("common.maximize")}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" strokeWidth={2} /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleClose} className="h-9 w-11 rounded-none text-muted-foreground hover:bg-red-600 hover:text-white" title={t("common.close")}>
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
        </Button>
      </div>
    </div>
  );
};

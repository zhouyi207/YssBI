import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscDatabase } from 'react-icons/vsc';
import { logger } from '@/utils/appLogger';
import { Button } from '@/components/ui/button';

interface TitleBarProps {
  isModified: boolean;
}

export const TitleBar: React.FC<TitleBarProps> = ({ isModified }) => {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const setup = async () => {
      const win = getCurrentWindow();
      setIsMaximized(await win.isMaximized());
      return await win.onResized(async () => {
        setIsMaximized(await win.isMaximized());
      });
    };
    let cleanup: (() => void) | null = null;
    setup().then(u => { cleanup = u; });
    return () => { if (cleanup) cleanup(); };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize();
  const handleMaximize = () => getCurrentWindow().toggleMaximize();
  const handleClose = () => getCurrentWindow().close();

  return (
    <div data-tauri-drag-region className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none shrink-0">
      <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
        <VscDatabase className="text-[var(--accent-color)]" size={16} />
        <span className="text-white font-bold text-sm tracking-tight">Data Viewer</span>
        {isModified && (
          <span className="text-[9px] text-yellow-500 font-mono ml-1">(modified)</span>
        )}
      </div>
      <div className="flex items-center h-full">
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMinimize} className="h-10 rounded-none text-muted-foreground" title="Minimize">
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMaximize} className="h-10 rounded-none text-muted-foreground" title={isMaximized ? 'Restore' : 'Maximize'}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" strokeWidth={2} /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleClose} className="h-10 w-12 rounded-none text-muted-foreground hover:bg-red-600 hover:text-white" title="Close">
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
        </Button>
      </div>
    </div>
  );
};

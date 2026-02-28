import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscDatabase } from 'react-icons/vsc';
import { logger } from '@/utils/appLogger';

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
        <button onClick={handleMinimize} className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors" title="Minimize">
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" /></svg>
        </button>
        <button onClick={handleMaximize} className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors" title={isMaximized ? 'Restore' : 'Maximize'}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" strokeWidth={2} /></svg>
        </button>
        <button onClick={handleClose} className="w-12 h-10 flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white transition-colors" title="Close">
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      </div>
    </div>
  );
};

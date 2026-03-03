import React, { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '@/utils/appLogger';
import Scatter, { type ScatterPoint } from '@/views/PlotView/Scatter';

interface ScatterPlotData {
  data: ScatterPoint[];
  x_label?: string;
  y_label?: string;
}

function getDataKeyFromHash(): string | null {
  const hash = window.location.hash;
  const match = hash.match(/[?&]key=([^&]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

/**
 * Plot 窗口组件
 * 用于显示数据可视化图表（散点图等）
 */
export const PlotWindow: React.FC = () => {
  const [isReady, setIsReady] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const [scatterData, setScatterData] = useState<ScatterPlotData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    let mounted = true;

    const initializeWindow = async () => {
      try {
        logger.sys.debug('Initializing plot window...', 'PlotWindow');
        const currentWindow = getCurrentWindow();
        const dataKey = getDataKeyFromHash();

        if (dataKey) {
          const json = await invoke<string | null>('get_window_data', { key: dataKey });
          if (mounted && json) {
            try {
              const parsed = JSON.parse(json) as ScatterPlotData;
              if (parsed.data && Array.isArray(parsed.data)) {
                setScatterData(parsed);
              } else {
                setError('Invalid scatter data format');
              }
            } catch (e) {
              setError(`Failed to parse data: ${e instanceof Error ? e.message : String(e)}`);
            }
          } else if (mounted) {
            setError('No data available for this window');
          }
        }

        if (mounted) setIsReady(true);
        await currentWindow.show().catch(() => {});

        const maximized = await currentWindow.isMaximized().catch(() => false);
        if (mounted) setIsMaximized(maximized);

        const unlisten = await currentWindow.onResized(async () => {
          if (!mounted) return;
          try {
            const max = await currentWindow.isMaximized();
            if (mounted) setIsMaximized(max);
          } catch (e) {
            logger.sys.warn('Failed to check maximized state on resize: ' + String(e), 'PlotWindow');
          }
        });
        if (mounted) cleanup = () => unlisten();

        logger.sys.debug('Plot window initialized successfully', 'PlotWindow');
      } catch (e) {
        logger.sys.error('Failed to initialize plot window: ' + String(e), 'PlotWindow');
        if (mounted) {
          setIsReady(true);
          setError('Failed to initialize window');
        }
      }
    };

    initializeWindow();

    return () => {
      mounted = false;
      if (cleanup) {
        cleanup();
      }
    };
  }, []);

  const handleMinimize = async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.minimize();
    } catch (e) {
      logger.app.error('Failed to minimize window: ' + String(e), 'PlotWindow');
    }
  };

  const handleMaximize = async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.toggleMaximize();
    } catch (e) {
      logger.app.error('Failed to maximize window: ' + String(e), 'PlotWindow');
    }
  };

  const handleClose = async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.close();
    } catch (e) {
      logger.app.error('Failed to close window: ' + String(e), 'PlotWindow');
    }
  };

  if (!isReady) {
    return (
      <div className="flex items-center justify-center w-full h-screen bg-[var(--workbench-bg)] text-gray-400">
        正在初始化...
      </div>
    );
  }

  return (
    <div className="flex flex-col w-full h-screen overflow-hidden bg-[var(--workbench-bg)]">
      {/* 自定义标题栏 - 与主窗口一致 */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none rounded-tr-lg overflow-hidden"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
          <span className="text-white font-bold text-sm tracking-tight">Plot</span>
        </div>

        {/* 窗口控制按钮 */}
        <div className="flex items-center h-full">
          <button
            onClick={handleMinimize}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
            title="最小化"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
            </svg>
          </button>
          <button
            onClick={handleMaximize}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
            title={isMaximized ? '还原' : '最大化'}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
            </svg>
          </button>
          <button
            onClick={handleClose}
            className="w-12 h-10 flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white transition-colors"
            title="关闭"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      {/* 主内容区：图表填充可用空间 */}
      <div className="flex-1 flex flex-col min-h-0 p-4">
        {error ? (
          <div className="flex flex-1 flex-col items-center justify-center text-gray-400 gap-3">
            <svg className="w-12 h-12 text-red-500/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
            </svg>
            <span className="text-sm">{error}</span>
          </div>
        ) : scatterData ? (
          <div className="flex-1 min-h-0 w-full">
            <Scatter
              data={scatterData.data}
              xLabel={scatterData.x_label}
              yLabel={scatterData.y_label}
            />
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center text-center">
            <div className="mb-4">
              <svg className="w-24 h-24 mx-auto text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            </div>
            <h2 className="text-2xl font-bold mb-2 text-white">Plot 窗口已就绪</h2>
            <p className="mb-4 text-gray-400">从 Scatter 节点执行后将在此显示散点图</p>
          </div>
        )}
      </div>
    </div>
  );
};
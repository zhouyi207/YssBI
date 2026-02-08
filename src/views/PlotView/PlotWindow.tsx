import React, { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Plot 窗口组件
 * 用于显示数据可视化图表
 */
export const PlotWindow: React.FC = () => {
  const [isReady, setIsReady] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    let mounted = true;

    const initializeWindow = async () => {
      try {
        console.log('[PlotWindow] Initializing plot window...');
        
        // 立即设置为ready
        if (mounted) {
          setIsReady(true);
        }
        
        // 异步初始化窗口功能
        setTimeout(async () => {
          if (!mounted) return;
          
          try {
            const currentWindow = getCurrentWindow();
            
            // 检查初始最大化状态
            const maximized = await currentWindow.isMaximized().catch(() => false);
            if (mounted) {
              setIsMaximized(maximized);
            }

            // 设置resize监听器
            let resizeTimeout: number;
            const unlisten = await currentWindow.onResized(async () => {
              clearTimeout(resizeTimeout);
              resizeTimeout = window.setTimeout(async () => {
                if (!mounted) return;
                try {
                  const maximized = await currentWindow.isMaximized();
                  if (mounted) {
                    setIsMaximized(maximized);
                  }
                } catch (e) {
                  console.warn('[PlotWindow] Failed to check maximized state on resize:', e);
                }
              }, 100);
            });
            
            if (mounted) {
              cleanup = () => {
                clearTimeout(resizeTimeout);
                unlisten();
              };
            }

            console.log('[PlotWindow] Plot window initialized successfully');
          } catch (e) {
            console.warn('[PlotWindow] Failed to setup window listeners:', e);
          }
        }, 100);

      } catch (e) {
        console.error('[PlotWindow] Failed to initialize window:', e);
        if (mounted) {
          setIsReady(true);
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
      console.error('Failed to minimize window:', e);
    }
  };

  const handleMaximize = async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.toggleMaximize();
    } catch (e) {
      console.error('Failed to maximize window:', e);
    }
  };

  const handleClose = async () => {
    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.close();
    } catch (e) {
      console.error('Failed to close window:', e);
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

      {/* 主内容区 */}
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="text-center">
          <div className="mb-4">
            <svg
              className="w-24 h-24 mx-auto text-[var(--accent-color)]"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
              />
            </svg>
          </div>
          <h2 className="text-2xl font-bold mb-2 text-white">
            Plot 窗口已就绪
          </h2>
          <p className="mb-4 text-gray-400">
            这是一个独立的可视化窗口
          </p>
          <div className="inline-flex items-center px-4 py-2 rounded-lg bg-[var(--accent-color)]/10">
            <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse mr-2"></div>
            <span className="text-sm text-gray-400">
              窗口活跃
            </span>
          </div>
          
          {/* 未来可以在这里添加图表库，如 Chart.js, ECharts 等 */}
          <div className="mt-8 text-sm text-gray-500">
            <p>提示：未来可以在这里显示：</p>
            <ul className="mt-2 space-y-1">
              <li>• 折线图、柱状图、散点图</li>
              <li>• 数据表格</li>
              <li>• 实时数据流</li>
              <li>• 自定义可视化组件</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
};
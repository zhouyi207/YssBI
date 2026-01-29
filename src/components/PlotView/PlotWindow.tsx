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
        
        // 确保CSS变量被正确设置
        const root = document.documentElement;
        if (!root.style.getPropertyValue('--workbench-bg')) {
          // 如果CSS变量未设置，使用默认值
          root.style.setProperty('--workbench-bg', '#121212');
          root.style.setProperty('--titlebar-bg', '#181818');
          root.style.setProperty('--border-color', '#252525');
          root.style.setProperty('--text-primary', '#ffffff');
          root.style.setProperty('--text-secondary', '#6b6b6b');
          root.style.setProperty('--hover-bg', 'rgba(255, 255, 255, 0.05)');
          console.log('[PlotWindow] Applied default CSS variables');
        }
        
        // 立即设置为ready，避免阻塞
        if (mounted) {
          setIsReady(true);
        }
        
        // 异步初始化窗口功能，不阻塞渲染
        setTimeout(async () => {
          if (!mounted) return;
          
          try {
            const currentWindow = getCurrentWindow();
            
            // 检查初始最大化状态
            const maximized = await currentWindow.isMaximized().catch(() => false);
            if (mounted) {
              setIsMaximized(maximized);
            }

            // 设置简单的resize监听器，使用防抖
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
              }, 100); // 100ms防抖
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
        }, 100); // 延迟100ms初始化窗口功能

      } catch (e) {
        console.error('[PlotWindow] Failed to initialize window:', e);
        if (mounted) {
          setIsReady(true); // 即使出错也设置为ready
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

  // 如果还未准备好，显示简单的加载状态
  if (!isReady) {
    return (
      <div 
        className="flex items-center justify-center w-full h-screen"
        style={{ backgroundColor: '#121212', color: '#ffffff' }}
      >
        <div style={{ color: '#6b6b6b' }}>正在初始化...</div>
      </div>
    );
  }

  return (
    <div 
      className="flex flex-col w-full h-screen overflow-hidden"
      style={{ backgroundColor: 'var(--workbench-bg, #121212)' }}
    >
      {/* 自定义标题栏 */}
      <div 
        data-tauri-drag-region
        className="flex items-center justify-between h-10 px-3 select-none"
        style={{ 
          backgroundColor: 'var(--titlebar-bg, #181818)', 
          borderBottom: '1px solid var(--border-color, #252525)' 
        }}
      >
        {/* 标题 */}
        <div data-tauri-drag-region className="flex items-center gap-2 flex-1">
          <svg
            className="w-4 h-4 text-blue-500"
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
          <h1 
            className="text-sm font-medium"
            style={{ color: 'var(--text-primary, #ffffff)' }}
          >
            Plot Visualization
          </h1>
        </div>

        {/* 窗口控制按钮 */}
        <div className="flex items-center">
          {/* 最小化按钮 */}
          <button
            onClick={handleMinimize}
            className="flex items-center justify-center w-10 h-10 transition-colors hover:bg-white hover:bg-opacity-5"
            style={{ 
              color: 'var(--text-secondary, #6b6b6b)'
            }}
            title="最小化"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M20 12H4"
              />
            </svg>
          </button>

          {/* 最大化/还原按钮 */}
          <button
            onClick={handleMaximize}
            className="flex items-center justify-center w-10 h-10 transition-colors hover:bg-white hover:bg-opacity-5"
            style={{ 
              color: 'var(--text-secondary, #6b6b6b)'
            }}
            title={isMaximized ? '还原' : '最大化'}
          >
            {isMaximized ? (
              <svg
                className="w-4 h-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25"
                />
              </svg>
            ) : (
              <svg
                className="w-4 h-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"
                />
              </svg>
            )}
          </button>

          {/* 关闭按钮 */}
          <button
            onClick={handleClose}
            className="flex items-center justify-center w-10 h-10 transition-colors group hover:bg-red-500 hover:text-white"
            style={{ color: 'var(--text-secondary, #6b6b6b)' }}
            title="关闭"
          >
            <svg
              className="w-4 h-4 group-hover:text-white"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* 主内容区 */}
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="text-center">
          <div className="mb-4">
            <svg
              className="w-24 h-24 mx-auto text-blue-500"
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
          <h2 
            className="text-2xl font-bold mb-2"
            style={{ color: 'var(--text-primary, #ffffff)' }}
          >
            Plot 窗口已就绪
          </h2>
          <p 
            className="mb-4"
            style={{ color: 'var(--text-secondary, #6b6b6b)' }}
          >
            这是一个独立的可视化窗口
          </p>
          <div 
            className="inline-flex items-center px-4 py-2 rounded-lg"
            style={{ backgroundColor: 'rgba(59, 130, 246, 0.1)' }}
          >
            <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse mr-2"></div>
            <span 
              className="text-sm"
              style={{ color: 'var(--text-secondary, #6b6b6b)' }}
            >
              窗口活跃
            </span>
          </div>
          
          {/* 未来可以在这里添加图表库，如 Chart.js, ECharts 等 */}
          <div 
            className="mt-8 text-sm"
            style={{ color: 'var(--text-secondary, #6b6b6b)' }}
          >
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

export default PlotWindow;
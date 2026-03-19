import React, { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { OLSComponent, type OLSResultData } from './OLSComponent';
import { VARComponent } from './VARComponent';
import { VECComponent } from './VECComponent';
import { DFADFComponent } from './DFADFComponent';
import { DFADFSummaryListComponent } from './DFADFSummaryListComponent';
import { BinaryComponent } from './BinaryComponent';
import { PanelComponent } from './PanelComponent';
import { PraisComponent, type PraisResultData } from './PraisComponent';
import { TwoSLSComponent } from './2SLSComponent';
import { LIMLComponent } from './LIMLComponent';
import { DataViewComponent } from './DataViewComponent';
import type { PanelSummaryResult } from './shared/types';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { logger } from '@/utils/appLogger';

function isDataView(data: unknown): data is { viewType: 'data_view'; [k: string]: unknown } {
  const d = data as Record<string, unknown>;
  return typeof d === 'object' && d != null && d.viewType === 'data_view';
}

function isVARSummary(data: unknown): data is { title: string; var_names?: string[]; oirf?: unknown } {
  const d = data as Record<string, unknown>;
  return typeof d === 'object' && d != null && Array.isArray(d.var_names) && d.oirf !== undefined;
}

function isVECSummary(data: unknown): data is { title: string; var_names?: string[]; rank?: number; trend_spec?: string } {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    Array.isArray(d.var_names) &&
    typeof d.rank === 'number' &&
    typeof d.trend_spec === 'string'
  );
}

function isDFADFSummaryList(data: unknown): data is { title: string; var_name: string; items: unknown[] } {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    Array.isArray(d.items) &&
    typeof d.var_name === 'string'
  );
}

function isDFADFSummary(data: unknown): data is { title: string; test_statistic: number; critical_value_5pct: number } {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    typeof d.test_statistic === 'number' &&
    typeof d.critical_value_5pct === 'number' &&
    d.oirf === undefined &&
    !Array.isArray(d.items)
  );
}

function isPanelSummary(data: unknown): data is PanelSummaryResult {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    (d.fe !== undefined || d.lsdv !== undefined || d.fd !== undefined || d.re !== undefined)
  );
}

function getDataKeyFromHash(): string | null {
  const hash = window.location.hash;
  const match = hash.match(/[?&]key=([^&]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

export const InfoWindow: React.FC = () => {
  const [isReady, setIsReady] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const [olsData, setOlsData] = useState<OLSResultData | PanelSummaryResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cleanup: (() => void)[] = [];
    let mounted = true;

    const initializeWindow = async () => {
      try {
        const currentWindow = getCurrentWindow();
        const dataKey = getDataKeyFromHash();

        if (!dataKey) {
          setError('Missing data key in URL');
          setIsReady(true);
          await currentWindow.show().catch(() => {});
          return;
        }

        const json = await invoke<string | null>('get_window_data', { key: dataKey });
        if (!mounted) return;

        if (json) {
          try {
            const parsed = JSON.parse(json);
            setOlsData(parsed);
            if (parsed?.title) {
              currentWindow.setTitle(parsed.title).catch(() => {});
            }
          } catch (e) {
            setError(`Failed to parse data: ${e instanceof Error ? e.message : String(e)}`);
          }
        } else {
          setError('No data available for this window');
        }

        await currentWindow.show().catch((e) =>
          logger.app.error('Failed to show window: ' + String(e), 'InfoWindow')
        );

        if (mounted) setIsReady(true);

        const maximized = await currentWindow.isMaximized().catch(() => false);
        if (mounted) setIsMaximized(maximized);

        const unlistenResize = await currentWindow.onResized(async () => {
          if (!mounted) return;
          try {
            const max = await currentWindow.isMaximized();
            if (mounted) setIsMaximized(max);
          } catch (e) {
            logger.sys.warn('Failed to check maximized state: ' + String(e), 'InfoWindow');
          }
        });
        cleanup.push(unlistenResize);
      } catch (e) {
        logger.sys.error('Failed to initialize window: ' + String(e), 'InfoWindow');
        if (mounted) {
          setIsReady(true);
          setError('Failed to initialize window');
        }
      }
    };

    initializeWindow();
    return () => {
      mounted = false;
      cleanup.forEach((fn) => fn());
    };
  }, []);

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (e) {
      logger.app.error('Failed to minimize: ' + String(e), 'InfoWindow');
    }
  };

  const handleMaximize = async () => {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (e) {
      logger.app.error('Failed to maximize: ' + String(e), 'InfoWindow');
    }
  };

  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch (e) {
      logger.app.error('Failed to close: ' + String(e), 'InfoWindow');
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
      {/* Title bar */}
      <div
        data-tauri-drag-region
        className="h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none rounded-tr-lg overflow-hidden shrink-0"
      >
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span className="text-white font-bold text-sm tracking-tight">
            {(olsData as { title?: string })?.title ?? 'Regression Results'}
          </span>
        </div>

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

      {/* Content */}
      <OverlayScrollbar className="flex-1 min-h-0" direction="vertical">
        {error ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-400 gap-3">
            <svg className="w-12 h-12 text-red-500/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
            </svg>
            <span className="text-sm">{error}</span>
          </div>
        ) : olsData ? (
          isDataView(olsData) ? (
            <DataViewComponent data={olsData} />
          ) : isDFADFSummaryList(olsData) ? (
            <DFADFSummaryListComponent data={olsData} />
          ) : isDFADFSummary(olsData) ? (
            <DFADFComponent data={olsData} />
          ) : isVECSummary(olsData) ? (
            <VECComponent data={olsData} />
          ) : isVARSummary(olsData) ? (
            <VARComponent data={olsData} />
          ) : isPanelSummary(olsData) ? (
            <PanelComponent data={olsData as PanelSummaryResult} />
          ) : olsData.diagnostic_info?.prais_info ? (
            <PraisComponent data={olsData as PraisResultData} />
          ) : olsData.model_basic_info?.model_type === 'Logit' ||
            olsData.model_basic_info?.model_type === 'Probit' ? (
            <BinaryComponent data={olsData} />
          ) : olsData.model_basic_info?.model_type === 'IV:2SLS' ? (
            <TwoSLSComponent data={olsData} />
          ) : olsData.model_basic_info?.model_type === 'IV:LIML' ? (
            <LIMLComponent data={olsData} />
          ) : (
            <OLSComponent data={olsData} />
          )
        ) : (
          <div className="flex items-center justify-center h-full text-gray-400">
            <div className="w-5 h-5 border-2 border-gray-600 border-t-[var(--accent-color)] rounded-full animate-spin" />
          </div>
        )}
      </OverlayScrollbar>
    </div>
  );
};

import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { SourceService, type SourceDescriptor } from '@/features/core/dataView';
import { usePersistedWindow, useWindowMaximized } from '@/features/application/window';
import { OLSComponent } from './OLSComponent';
import { VARComponent } from './VARComponent';
import { VARSocComponent } from './VARSocComponent';
import { VECComponent } from './VECComponent';
import { VecRankComponent } from './VecRankComponent';
import { DFADFComponent } from './DFADFComponent';
import { DFADFSummaryListComponent } from './DFADFSummaryListComponent';
import { BinaryComponent } from './BinaryComponent';
import { PanelComponent } from './PanelComponent';
import { DIDComponent } from './DIDComponent';
import { PraisComponent, type PraisResultData } from './PraisComponent';
import { TwoSLSComponent } from './2SLSComponent';
import { LIMLComponent } from './LIMLComponent';
import { DataViewComponent } from './DataViewComponent';
import type { PanelDidResultData, PanelSummaryResult, VecRankResultData } from './shared/types';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';
import { logger } from '@/utils/appLogger';

function isDataView(data: unknown): data is SourceDescriptor {
  const d = data as SourceDescriptor;
  return (
    typeof d === 'object' &&
    d != null &&
    ['dataframe', 'series', 'scalar', 'null', 'json'].includes(d.renderer)
  );
}

function isVARSummary(data: unknown): data is { title: string; var_names?: string[]; oirf?: unknown } {
  const d = data as Record<string, unknown>;
  return typeof d === 'object' && d != null && Array.isArray(d.var_names) && d.oirf !== undefined;
}

function isVARSoc(data: unknown): data is { title: string; maxlag: number; rows: unknown[] } {
  const d = data as Record<string, unknown>;
  if (typeof d !== 'object' || d == null || typeof d.maxlag !== 'number' || !Array.isArray(d.rows)) {
    return false;
  }
  const r0 = d.rows[0] as Record<string, unknown> | undefined;
  return (
    r0 != null &&
    typeof r0.lag === 'number' &&
    typeof r0.log_likelihood === 'number' &&
    d.oirf === undefined
  );
}

function isVecRank(data: unknown): data is VecRankResultData {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    d.kind === 'vecrank' &&
    Array.isArray(d.rows) &&
    typeof d.num_observation === 'number' &&
    typeof d.n_lags === 'number'
  );
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

function isPanelDid(data: unknown): data is PanelDidResultData {
  const d = data as Record<string, unknown>;
  return typeof d === 'object' && d != null && d.kind === 'panel_did';
}

function isPanelSummary(data: unknown): data is PanelSummaryResult {
  const d = data as Record<string, unknown>;
  return (
    typeof d === 'object' &&
    d != null &&
    d.kind !== 'panel_did' &&
    (d.fe !== undefined || d.lsdv !== undefined || d.fd !== undefined || d.re !== undefined)
  );
}

function getSourceIdFromHash(): string | null {
  const hash = window.location.hash;
  const match = hash.match(/[?&]sourceId=([^&]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

export const InfoWindow: React.FC = () => {
  const { t } = useTranslation();
  const [isReady, setIsReady] = useState(false);
  const isMaximized = useWindowMaximized('InfoWindow');
  const [olsData, setOlsData] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);

  usePersistedWindow('info');

  useEffect(() => {
    let mounted = true;

    const initializeWindow = async () => {
      try {
        const currentWindow = getCurrentWindow();
        const sourceId = getSourceIdFromHash();

        if (!sourceId) {
          setError(t('info.missingDataKey'));
          setIsReady(true);
          await currentWindow.show().catch(() => {});
          return;
        }

        const metadata = await SourceService.getDescriptor(sourceId);
        if (!mounted) return;

        if (metadata) {
          currentWindow.setTitle(metadata.title).catch(() => {});

          if (isDataView(metadata)) {
            setOlsData(metadata);
          } else {
            const value = await SourceService.getValue(metadata.sourceId);
            if (!mounted) return;
            if (!value) {
              setError(t('info.noData'));
              return;
            }
            setOlsData(value.value ?? value.structured ?? value);
          }
        } else {
            setError(t('info.noData'));
        }

        await currentWindow.show().catch((e) =>
          logger.app.error('Failed to show window: ' + String(e), 'InfoWindow')
        );

        if (mounted) setIsReady(true);
      } catch (e) {
        logger.sys.error('Failed to initialize window: ' + String(e), 'InfoWindow');
        if (mounted) {
          setIsReady(true);
          setError(t('info.failedInitialize'));
        }
      }
    };

    initializeWindow();
    return () => {
      mounted = false;
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
      <div className="flex items-center justify-center w-full h-screen bg-[var(--workbench-bg)] text-muted-foreground">
        {t('common.initializing')}
      </div>
    );
  }

  const resultData = olsData as any;

  return (
    <div className="flex flex-col w-full h-screen overflow-hidden bg-[var(--workbench-bg)] text-foreground">
      {/* Title bar */}
      <WindowTitleBar childWindow>
        <div className="flex items-center gap-2 px-4 flex-1" data-tauri-drag-region>
          <svg className="w-4 h-4 text-[var(--accent-color)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span className="text-foreground font-bold text-sm tracking-tight">
            {resultData?.title ?? t('info.regressionResults')}
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

      {/* Content */}
      <OverlayScrollbar className="flex-1 min-h-0" direction="vertical">
        {error ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3">
            <svg className="w-12 h-12 text-red-500/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
            </svg>
            <span className="text-sm">{error}</span>
          </div>
        ) : resultData ? (
          isDataView(resultData) ? (
            <DataViewComponent data={resultData as any} />
          ) : isDFADFSummaryList(resultData) ? (
            <DFADFSummaryListComponent data={resultData as any} />
          ) : isDFADFSummary(resultData) ? (
            <DFADFComponent data={resultData as any} />
          ) : isVecRank(resultData) ? (
            <VecRankComponent data={resultData} />
          ) : isVECSummary(resultData) ? (
            <VECComponent data={resultData as any} />
          ) : isVARSoc(resultData) ? (
            <VARSocComponent data={resultData as any} />
          ) : isVARSummary(resultData) ? (
            <VARComponent data={resultData as any} />
          ) : isPanelDid(resultData) ? (
            <DIDComponent data={resultData} />
          ) : isPanelSummary(resultData) ? (
            <PanelComponent data={resultData as PanelSummaryResult} />
          ) : resultData.diagnostic_info?.prais_info ? (
            <PraisComponent data={resultData as PraisResultData} />
          ) : resultData.model_basic_info?.model_type === 'Logit' ||
            resultData.model_basic_info?.model_type === 'Probit' ? (
            <BinaryComponent data={resultData} />
          ) : resultData.model_basic_info?.model_type === 'IV:2SLS' ? (
            <TwoSLSComponent data={resultData} />
          ) : resultData.model_basic_info?.model_type === 'IV:LIML' ? (
            <LIMLComponent data={resultData} />
          ) : (
            <OLSComponent data={resultData} />
          )
        ) : (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <div className="w-5 h-5 border-2 border-border border-t-[var(--accent-color)] rounded-full animate-spin" />
          </div>
        )}
      </OverlayScrollbar>
    </div>
  );
};

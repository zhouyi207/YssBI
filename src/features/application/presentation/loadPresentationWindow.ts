import { SourceService } from '@/services/resultSource/resultSourceService';
import type {
  PlotChart,
  ReportKind,
  SourceDescriptor,
  SourceValue,
} from '@/features/core/resultSource';
import { parsePlotChartFromLocation } from './parsePresentationWindowQuery';

export type PresentationWindowState =
  | { status: 'loading' }
  | { status: 'missing_source_id' }
  | { status: 'not_found' }
  | { status: 'load_failed'; message: string }
  | { status: 'ready'; descriptor: SourceDescriptor; payload: PresentationPayload };

export type PresentationPayload =
  | { mode: 'inspector'; descriptor: SourceDescriptor }
  | { mode: 'plot'; chart: PlotChart; data: unknown }
  | { mode: 'report'; report: ReportKind; data: unknown };

function extractPayload(value: SourceValue): unknown {
  return value.value ?? value.structured ?? value;
}

function resolvePlotChart(descriptor: SourceDescriptor): PlotChart {
  if (descriptor.presentation.kind === 'plot') {
    return descriptor.presentation.chart;
  }
  const fallback = parsePlotChartFromLocation();
  const allowed: PlotChart[] = [
    'scatter',
    'line',
    'plot',
    'ecdf',
    'kde',
    'histogram',
    'correlation',
    'correlogram',
  ];
  if (fallback && allowed.includes(fallback as PlotChart)) {
    return fallback as PlotChart;
  }
  return 'scatter';
}

export async function loadPresentationWindow(
  sourceId: string,
): Promise<PresentationWindowState> {
  if (!sourceId.trim()) {
    return { status: 'missing_source_id' };
  }

  try {
    const descriptor = await SourceService.getDescriptor(sourceId);
    if (!descriptor) {
      return { status: 'not_found' };
    }

    switch (descriptor.presentation.kind) {
      case 'inspector':
        return {
          status: 'ready',
          descriptor,
          payload: { mode: 'inspector', descriptor },
        };

      case 'plot': {
        const value = await SourceService.getValue(descriptor.sourceId);
        if (!value) return { status: 'not_found' };
        return {
          status: 'ready',
          descriptor,
          payload: {
            mode: 'plot',
            chart: resolvePlotChart(descriptor),
            data: extractPayload(value),
          },
        };
      }

      case 'report': {
        const value = await SourceService.getValue(descriptor.sourceId);
        if (!value) return { status: 'not_found' };
        return {
          status: 'ready',
          descriptor,
          payload: {
            mode: 'report',
            report: descriptor.presentation.report,
            data: extractPayload(value),
          },
        };
      }
    }
  } catch (error) {
    return {
      status: 'load_failed',
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

import { ResultService } from '@/services/result/resultService';
import type {
  ResultDescriptor,
  ResultFailure,
  ResultPage,
  ResultPlotKind,
  ResultProgress,
  ResultReportKind,
} from '@/shared/types/dto/result';
import { logger } from '@/utils/appLogger';
import { parsePlotChartFromLocation } from './parsePresentationWindowQuery';

export type PresentationWindowState =
  | { status: 'loading' }
  | { status: 'missing_result_id' }
  | { status: 'not_found' }
  | { status: 'pending'; descriptor: ResultDescriptor; progress: ResultProgress }
  | { status: 'failed'; descriptor: ResultDescriptor; failure: ResultFailure }
  | { status: 'cancelled'; descriptor: ResultDescriptor }
  | { status: 'load_failed' }
  | { status: 'ready'; descriptor: ResultDescriptor; payload: PresentationPayload };

export type PresentationPayload =
  | { mode: 'inspector'; descriptor: ResultDescriptor; page?: ResultPage }
  | { mode: 'plot'; chart: ResultPlotKind; data: unknown }
  | { mode: 'report'; report: ResultReportKind; data: unknown };

const PAGE_SIZE = 200;

function resolvePlotChart(descriptor: ResultDescriptor): ResultPlotKind {
  if (descriptor.presentation.kind === 'plot') return descriptor.presentation.chart;
  const fallback = parsePlotChartFromLocation();
  const allowed: ResultPlotKind[] = [
    'scatter', 'line', 'plot', 'ecdf', 'kde', 'histogram', 'correlation', 'correlogram',
  ];
  return allowed.includes(fallback as ResultPlotKind) ? fallback as ResultPlotKind : 'scatter';
}

async function loadReadyPayload(descriptor: ResultDescriptor): Promise<PresentationPayload> {
  if (descriptor.presentation.kind === 'inspector') {
    if (descriptor.valueKind === 'scalar') {
      await ResultService.getValue(descriptor.resultId);
      return { mode: 'inspector', descriptor };
    }
    const page = await ResultService.getPage(descriptor.resultId, 0, PAGE_SIZE);
    if (!page) throw new Error('Result data was not found');
    return { mode: 'inspector', descriptor, page };
  }

  if (descriptor.valueKind === 'scalar') {
    const value = await ResultService.getValue(descriptor.resultId);
    if (!value || value.kind !== 'value') {
      throw new Error('Presentation results require a canonical scalar value');
    }
    return descriptor.presentation.kind === 'plot'
      ? { mode: 'plot', chart: resolvePlotChart(descriptor), data: value.value }
      : { mode: 'report', report: descriptor.presentation.report, data: value.value };
  }

  const page = await ResultService.getPage(descriptor.resultId, 0, PAGE_SIZE);
  if (!page) throw new Error('Result data was not found');
  if (descriptor.presentation.kind === 'report') {
    throw new Error('Report results require a canonical scalar object');
  }
  return { mode: 'plot', chart: resolvePlotChart(descriptor), data: page.values };
}

export async function loadPresentationWindow(resultId: string): Promise<PresentationWindowState> {
  if (!resultId.trim()) return { status: 'missing_result_id' };
  try {
    const descriptor = await ResultService.getDescriptor(resultId);
    if (!descriptor) return { status: 'not_found' };
    switch (descriptor.state.kind) {
      case 'pending':
        return { status: 'pending', descriptor, progress: descriptor.state.progress };
      case 'failed':
        return { status: 'failed', descriptor, failure: descriptor.state.failure };
      case 'cancelled':
        return { status: 'cancelled', descriptor };
      case 'ready':
        return { status: 'ready', descriptor, payload: await loadReadyPayload(descriptor) };
    }
  } catch (error) {
    logger.app.error(
      `Failed to load presentation result: ${error instanceof Error ? error.message : String(error)}`,
      'loadPresentationWindow',
    );
    return { status: 'load_failed' };
  }
}

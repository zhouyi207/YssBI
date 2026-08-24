import { beforeEach, describe, expect, it } from 'vitest';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { LOG_DOMAIN_ORDER } from './logDomains';
import { applyLogFilter, type DiagnosticLogFilter, useLogStore } from './logStore';

function record(
  domain: DiagnosticRecordDto['domain'],
  sequence: number,
  level: DiagnosticRecordDto['level'] = 'info',
  message = `${domain} message`,
): DiagnosticRecordDto {
  return {
    streamId: 'stream-1',
    sequence,
    timestamp: '2026-08-16T10:11:12.000Z',
    level,
    origin: 'rust',
    domain,
    target: `${domain}.target`,
    message,
    fields: {},
  };
}

const allLevels = new Set<DiagnosticRecordDto['level']>([
  'trace',
  'debug',
  'info',
  'warn',
  'error',
]);

describe('diagnostic log domain filtering', () => {
  beforeEach(() => {
    useLogStore.setState({
      filter: { levels: new Set(allLevels), searchText: '' },
      selectedLog: null,
      autoScroll: true,
    });
  });

  it('defines domains from the backend contract, including ui and excluding notify', () => {
    expect(LOG_DOMAIN_ORDER).toEqual([
      'all',
      'application',
      'execution',
      'system',
      'graph',
      'data',
      'ui',
    ]);
  });

  it('applies shared level and search filters independently per domain', () => {
    const logs = [
      record('graph', 1, 'info', 'graph ready'),
      record('application', 2, 'info', 'application ready'),
      record('graph', 3, 'warn', 'graph ready with warning'),
      record('graph', 4, 'info', 'graph waiting'),
    ];
    const filter: DiagnosticLogFilter = {
      levels: new Set(['info']),
      searchText: 'ready',
    };

    expect(applyLogFilter(logs, filter, 'graph').map((log) => log.domain))
      .toEqual(['graph']);
    expect(applyLogFilter(logs, filter, 'all').map((log) => log.domain))
      .toEqual(['graph', 'application']);
  });

  it('preserves autoScroll when the main Logs consumer is reopened', () => {
    useLogStore.getState().setAutoScroll(false);

    useLogStore.getState().setSearchText('ready');
    const reopenedState = useLogStore.getState();

    expect(reopenedState.autoScroll).toBe(false);
    expect(reopenedState.filter.searchText).toBe('ready');
  });
});

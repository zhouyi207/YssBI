import { describe, expect, it } from 'vitest';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { LOG_DOMAIN_TAB_ORDER } from './logDomainTabs';
import { applyLogFilter, type DiagnosticLogFilter } from './logStore';

function record(domain: DiagnosticRecordDto['domain'], sequence: number): DiagnosticRecordDto {
  return {
    streamId: 'stream-1',
    sequence,
    timestamp: '2026-08-16T10:11:12.000Z',
    level: 'info',
    origin: 'rust',
    domain,
    target: `${domain}.target`,
    message: `${domain} message`,
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
  it('defines tabs from the backend domain contract, including ui and excluding notify', () => {
    expect(LOG_DOMAIN_TAB_ORDER).toEqual([
      'all',
      'application',
      'execution',
      'system',
      'graph',
      'data',
      'ui',
    ]);
  });

  it('filters records by domain without a legacy log type field', () => {
    const filter: DiagnosticLogFilter = {
      levels: allLevels,
      domains: new Set(['graph']),
      searchText: '',
    };
    expect(applyLogFilter([record('application', 1), record('graph', 2)], filter))
      .toEqual([record('graph', 2)]);
  });
});

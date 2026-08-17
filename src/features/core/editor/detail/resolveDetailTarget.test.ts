import { describe, expect, it } from 'vitest';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { resolveDetailTarget } from './resolveDetailTarget';

const logEntry = {
  streamId: 'stream-1',
  sequence: 1,
  timestamp: '2026-08-16T10:11:12.000Z',
  level: 'info',
  origin: 'frontend',
  domain: 'application',
  target: 'test',
  message: 'test',
  fields: {},
} satisfies DiagnosticRecordDto;

describe('resolveDetailTarget', () => {
  it('returns explicit detail focus when set', () => {
    expect(
      resolveDetailTarget({
        detailFocus: { kind: 'variable', id: 'var-1' },
        selectedLog: null,
      }),
    ).toEqual({ kind: 'variable', id: 'var-1' });
  });

  it('returns event detail focus from sidebar click', () => {
    expect(
      resolveDetailTarget({
        detailFocus: { kind: 'event', path: 'g1' },
        selectedLog: null,
      }),
    ).toEqual({ kind: 'event', path: 'g1' });
  });

  it('returns node detail focus without competing with active tab', () => {
    expect(
      resolveDetailTarget({
        detailFocus: { kind: 'node', id: 'node-1' , graphPath: 'g1' },
        selectedLog: null,
      }),
    ).toEqual({ kind: 'node', id: 'node-1' , graphPath: 'g1' });
  });

  it('returns log detail when focus is log and a log is selected', () => {
    expect(
      resolveDetailTarget({
        detailFocus: { kind: 'log' },
        selectedLog: logEntry,
      }),
    ).toEqual({ kind: 'log' });
  });

  it('returns null when log focus is set but no log is selected', () => {
    expect(
      resolveDetailTarget({
        detailFocus: { kind: 'log' },
        selectedLog: null,
      }),
    ).toBeNull();
  });

  it('returns null when nothing is focused', () => {
    expect(
      resolveDetailTarget({
        detailFocus: null,
        selectedLog: logEntry,
      }),
    ).toBeNull();
  });
});

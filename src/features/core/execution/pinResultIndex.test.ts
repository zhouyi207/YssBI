import { describe, expect, it } from 'vitest';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import {
  lookupPinHistory,
  lookupPinPreview,
  pinHistoryCacheKey,
  pinPreviewCacheKey,
} from './pinResultIndex';

const graphPath = 'events/Main.yssbi-event';

describe('pinResultIndex', () => {
  it('keys previews by exact stable declared and dynamic addresses', () => {
    const declared: PortAddressDto = {
      kind: 'declared',
      nodeId: 'node-1',
      portKey: 'value',
    };
    const instance: PortAddressDto = {
      kind: 'instance',
      nodeId: 'node-1',
      templateKey: 'value',
      instanceId: 'instance-1',
    };
    const previews = new Map([
      [pinPreviewCacheKey(graphPath, declared), { port: declared }],
      [pinPreviewCacheKey(graphPath, instance), { port: instance }],
    ]);

    expect(pinPreviewCacheKey(graphPath, declared)).not.toBe(
      pinPreviewCacheKey(graphPath, instance),
    );
    expect(lookupPinPreview(previews, graphPath, instance)?.port).toEqual(instance);
  });

  it('distinguishes dynamic addresses by instance and template', () => {
    const base: PortAddressDto = {
      kind: 'instance',
      nodeId: 'node-1',
      templateKey: 'values',
      instanceId: 'instance-1',
    };
    const keys = new Set([
      pinHistoryCacheKey(graphPath, base),
      pinHistoryCacheKey(graphPath, { ...base, instanceId: 'instance-2' }),
      pinHistoryCacheKey(graphPath, { ...base, templateKey: 'weights' }),
    ]);
    expect(keys.size).toBe(3);
  });

  it('never falls back across graph paths', () => {
    const output: PortAddressDto = {
      kind: 'declared',
      nodeId: 'node-1',
      portKey: 'result',
    };
    const projection = {
      graphPath: 'events/a.yssbi-event',
      output,
      entries: [],
      selectedResultId: null,
    };
    const histories = new Map([[pinHistoryCacheKey(projection.graphPath, output), projection]]);

    expect(lookupPinHistory(histories, projection.graphPath, output)).toBe(projection);
    expect(lookupPinHistory(histories, 'events/b.yssbi-event', output)).toBeUndefined();
  });
});

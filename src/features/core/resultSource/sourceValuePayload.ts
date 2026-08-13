import type { SourceValue } from './types';

export function sourceValuePayload(source: SourceValue): unknown {
  return source.value;
}

export function reportSourceValuePayload(source: SourceValue): unknown {
  const payload = sourceValuePayload(source);
  if (source.kind === 'sequence' && Array.isArray(payload) && payload.length === 1) {
    return payload[0];
  }
  return payload;
}

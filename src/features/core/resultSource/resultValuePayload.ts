import type { ResultValue } from './types';

export function resultValuePayload(source: ResultValue): unknown {
  return source.value;
}

export function reportResultValuePayload(source: ResultValue): unknown {
  if (source.kind !== 'value') {
    throw new Error('Report results require a canonical value object');
  }
  return resultValuePayload(source);
}

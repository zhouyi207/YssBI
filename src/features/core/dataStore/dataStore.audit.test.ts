import { describe, expect, it } from 'vitest';
import { auditProjectStoreImports } from './projectStoreDeps';

describe('dataStore project lifecycle store imports', () => {
  it('lifecycle modules explicitly import every required store hook', () => {
    const failures = auditProjectStoreImports();
    expect(failures, JSON.stringify(failures, null, 2)).toEqual([]);
  });
});

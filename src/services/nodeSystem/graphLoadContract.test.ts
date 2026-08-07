import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const explicitlyAuditedGraphLoadModules = [
  '../../features/core/dataStore/projectIOStore.ts',
  '../../features/application/editorProjection/graphProjectionCoordinator.ts',
  './graphProjectionService.ts',
  '../../features/core/dataStore/graphDataStore.ts',
  '../../features/domain/editorProjection/toProjectionEntities.ts',
  '../../shared/types/dto/editorProjectionParser.ts',
] as const;

const legacyLoadSymbols = [
  'resolve_graph_dynamic_pins',
  'resolveEffectiveDefinition',
] as const;

describe('explicit production graph-load module audit', () => {
  it('keeps the currently listed modules free of legacy graph hydration symbols', () => {
    const offenders = explicitlyAuditedGraphLoadModules.flatMap((fileName) => {
      const source = readFileSync(new URL(fileName, import.meta.url), 'utf8');
      return legacyLoadSymbols
        .filter((symbol) => source.includes(symbol))
        .map((symbol) => `${fileName}: ${symbol}`);
    });

    expect(offenders).toEqual([]);
  });
});

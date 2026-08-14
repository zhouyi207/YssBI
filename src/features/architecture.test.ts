import { readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const productionExtensions = new Set(['.ts', '.tsx']);
const applicationImport = /(?:from\s*|import\s*)[('"`]@\/features\/application(?:\/|['"`])/;

function productionFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionFiles(path);
    if (!productionExtensions.has(extname(entry.name)) || entry.name.includes('.test.')) return [];
    return [path];
  });
}

describe('feature dependency architecture', () => {
  it('keeps production core independent from application', () => {
    const coreDirectory = join(process.cwd(), 'src', 'features', 'core');
    const violations = productionFiles(coreDirectory)
      .filter((path) => applicationImport.test(readFileSync(path, 'utf8')))
      .map((path) => relative(process.cwd(), path).replace(/\\/g, '/'));

    expect(violations).toEqual([]);
  });
});

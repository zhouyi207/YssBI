import { readdirSync } from 'node:fs';
import { extname, join, resolve } from 'node:path';
import * as ts from 'typescript/unstable/ast';
import { describe, expect, it } from 'vitest';
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';

const forbiddenFeedbackModules = new Set([
  'sonner',
  '@/components/ui/sonner',
  '@/shared/ui/Toast',
]);

function productionFiles(directory: string): string[] {
  return readdirSync(resolve(directory), { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionFiles(path);
    if (!['.ts', '.tsx'].includes(extname(path)) || /\.test\.[^.]+$/.test(path)) return [];
    return [path.replace(/\\/g, '/')];
  });
}

function isLoggerNotifyCall(node: ts.CallExpression): boolean {
  const method = node.expression;
  if (!ts.isPropertyAccessExpression(method)) return false;
  const notify = method.expression;
  return ts.isPropertyAccessExpression(notify)
    && notify.name.text === 'notify'
    && ts.isIdentifier(notify.expression)
    && notify.expression.text === 'logger';
}

function feedbackOffenders(path: string, sourceFile: ts.SourceFile): string[] {
  const offenders: string[] = [];
  function report(node: ts.Node, reason: string): void {
    const { line } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    offenders.push(`${path}:${line + 1}: ${reason}`);
  }
  function visit(node: ts.Node): void {
    if (ts.isCallExpression(node) && isLoggerNotifyCall(node)) {
      report(node, 'logger.notify is not a user-feedback channel');
    }
    if (ts.isImportDeclaration(node)
      && ts.isStringLiteral(node.moduleSpecifier)
      && forbiddenFeedbackModules.has(node.moduleSpecifier.text)) {
      report(node, `forbidden transient notification module ${node.moduleSpecifier.text}`);
    }
    node.forEachChild(visit);
  }
  visit(sourceFile);
  return offenders;
}

function fixtureOffenders(source: string): string[] {
  return withIsolatedTypeScriptProject({ 'fixture.ts': source }, ({ sourceFile }) => (
    feedbackOffenders('fixture.ts', sourceFile('fixture.ts'))
  ));
}

describe('user feedback architecture contract', () => {
  it.each([
    `logger.notify.info(t('notifications.saved'), 'UI');`,
    `import { toast } from 'sonner';`,
  ])('rejects feedback routed through removed global channels: %s', (source) => {
    expect(fixtureOffenders(source)).toEqual([expect.stringContaining('fixture.ts:1')]);
  });

  it('allows diagnostic logging', () => {
    expect(fixtureOffenders(`logger.app.error('diagnostic', 'ProjectPicker');`)).toEqual([]);
  });

  it('keeps production feedback out of diagnostic notification and toaster channels', () => {
    const files = productionFiles('src');
    const offenders = withProductionTypeScriptProject(({ sourceFile }) => files.flatMap((path) => (
      feedbackOffenders(path, sourceFile(path))
    )));
    expect(offenders).toEqual([]);
  });
});

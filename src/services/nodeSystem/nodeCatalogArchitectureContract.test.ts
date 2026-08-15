import { readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import * as ts from 'typescript/unstable/ast';
import { describe, expect, it } from 'vitest';
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';

const scopedDirectories = [
  'src/features/application/nodeCatalog',
  'src/features/core/nodeCatalog',
  'src/features/domain/nodeCatalog',
  'src/features/core/dnd',
  'src/features/application/editor/canvasDrop',
] as const;


const forbiddenIdentifiers = new Set([
  'NodeDefinition',
  'resolveEffectiveDefinition',
  'signatureToPinSlots',
  'buildBuiltinCatalogItems',
  'buildContextualCatalogItems',
  'searchNodeDocumentation',
  'NODE_CATALOG_UNAVAILABLE_MESSAGE',
]);

const forbiddenModuleBasenames = new Set([
  'buildBuiltinCatalogItems',
  'buildContextualCatalogItems',
  'searchNodeDocumentation',
  'resolveEffectiveDefinition',
]);

function productionFiles(directory: string): string[] {
  return readdirSync(resolve(directory), { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionFiles(path);
    if (!['.ts', '.tsx'].includes(extname(path)) || /\.test\.[^.]+$/.test(path)) return [];
    return [path.replace(/\\/g, '/')];
  });
}

function propertyName(node: ts.PropertyName | undefined): string | null {
  if (!node) return null;
  if (ts.isIdentifier(node) || ts.isStringLiteral(node)) return node.text;
  return null;
}

const descriptorIdentityFields = new Set([
  'nodeTypeId',
  'resourcePath',
  'resourceRevision',
  'createArgs',
]);

function expressionTailName(node: ts.Expression): string | null {
  if (ts.isIdentifier(node)) return node.text;
  if (ts.isPropertyAccessExpression(node)) return node.name.text;
  if (ts.isElementAccessExpression(node) && ts.isStringLiteral(node.argumentExpression)) {
    return node.argumentExpression.text;
  }
  return null;
}

function objectDescriptorShape(node: ts.ObjectLiteralExpression): {
  hasKind: boolean;
  identityFieldCount: number;
} {
  let hasKind = false;
  let identityFieldCount = 0;
  for (const property of node.properties) {
    if (ts.isSpreadAssignment(property)) continue;
    const name = propertyName(property.name);
    if (name === 'kind') hasKind = true;
    if (name && descriptorIdentityFields.has(name)) identityFieldCount += 1;
  }
  return { hasKind, identityFieldCount };
}

function descriptorSynthesisReason(node: ts.ObjectLiteralExpression): string | null {
  let hasKind = false;
  let hasIdentityField = false;

  for (const property of node.properties) {
    if (ts.isSpreadAssignment(property)) {
      const spreadName = expressionTailName(property.expression);
      if (spreadName === 'descriptor' || spreadName === 'creation') return 'descriptor spread';
      continue;
    }

    const name = propertyName(property.name);
    if (name === 'kind') {
      hasKind = true;
      if (ts.isPropertyAssignment(property)
        && ts.isStringLiteral(property.initializer)
        && (property.initializer.text === 'static' || property.initializer.text === 'resourceBound')) {
        return `${property.initializer.text} literal`;
      }
    }
    if (name && descriptorIdentityFields.has(name)) hasIdentityField = true;
  }

  return hasKind && hasIdentityField ? 'descriptor field reconstruction' : null;
}

type DescriptorAliasKind = 'descriptor' | 'identity';
type DescriptorAliasScope = Map<string, DescriptorAliasKind | null>;

function unwrapExpression(node: ts.Expression): ts.Expression {
  if (ts.isParenthesizedExpression(node)
    || ts.isAssertionExpression(node)
    || ts.isNonNullExpression(node)
    || ts.isSatisfiesExpression(node)) {
    return unwrapExpression(node.expression);
  }
  return node;
}

function descriptorAliasOffenders(path: string, sourceFile: ts.SourceFile): string[] {
  const offenders = new Set<string>();
  const scopes: DescriptorAliasScope[] = [new Map()];

  function resolveAlias(name: string): DescriptorAliasKind | null {
    for (let index = scopes.length - 1; index >= 0; index -= 1) {
      if (scopes[index].has(name)) return scopes[index].get(name) ?? null;
    }
    return null;
  }

  function classifyExpression(expression: ts.Expression): DescriptorAliasKind | null {
    const node = unwrapExpression(expression);
    const tailName = expressionTailName(node);
    if (tailName === 'descriptor' || tailName === 'creation') return 'descriptor';
    if (ts.isIdentifier(node)) return resolveAlias(node.text);
    if (ts.isObjectLiteralExpression(node)) {
      const shape = objectDescriptorShape(node);
      if (!shape.hasKind && shape.identityFieldCount >= 2) return 'identity';
    }
    return null;
  }

  function withScope(callback: () => void): void {
    scopes.push(new Map());
    callback();
    scopes.pop();
  }

  function bindVariable(node: ts.VariableDeclaration): void {
    if (!ts.isIdentifier(node.name)) return;
    const kind = node.initializer ? classifyExpression(node.initializer) : null;
    scopes[scopes.length - 1].set(node.name.text, kind);
  }

  function inspectSpreadReconstruction(node: ts.ObjectLiteralExpression): void {
    const shape = objectDescriptorShape(node);
    for (const property of node.properties) {
      if (!ts.isSpreadAssignment(property)) continue;
      const spreadExpression = unwrapExpression(property.expression);
      const directTailName = expressionTailName(spreadExpression);
      if (directTailName === 'descriptor' || directTailName === 'creation') continue;
      const aliasKind = classifyExpression(spreadExpression);
      if (aliasKind === 'descriptor' || (aliasKind === 'identity' && shape.hasKind)) {
        offenders.add(`${path}: synthesized creation descriptor (${aliasKind} alias spread)`);
      }
    }
  }

  function visit(node: ts.Node): void {
    if (ts.isFunctionLikeDeclaration(node)) {
      withScope(() => {
        for (const parameter of node.parameters) {
          if (ts.isIdentifier(parameter.name)) scopes[scopes.length - 1].set(parameter.name.text, null);
        }
        const body = (node as ts.SignatureDeclaration & { body?: ts.Node }).body;
        if (body) visit(body);
      });
      return;
    }
    if (ts.isBlock(node)) {
      withScope(() => node.statements.forEach(visit));
      return;
    }
    if (ts.isVariableDeclaration(node)) {
      if (node.initializer) visit(node.initializer);
      bindVariable(node);
      return;
    }
    if (ts.isObjectLiteralExpression(node)) inspectSpreadReconstruction(node);
    node.forEachChild(visit);
  }

  visit(sourceFile);
  return [...offenders];
}

function sourceOffenders(path: string, sourceFile: ts.SourceFile): string[] {
  const offenders = new Set<string>();

  function visit(node: ts.Node): void {
    if (ts.isIdentifier(node) && forbiddenIdentifiers.has(node.text)) {
      offenders.add(`${path}: forbidden identifier ${node.text}`);
    }
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node))
      && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      const segments = node.moduleSpecifier.text.split('/');
      const basename = segments[segments.length - 1] ?? '';
      if (forbiddenModuleBasenames.has(basename)) {
        offenders.add(`${path}: forbidden module ${node.moduleSpecifier.text}`);
      }
    }
    if (ts.isStringLiteral(node)
      && (node.text === 'sidebar.nodeCatalogUnavailable'
        || node.text === 'sidebar.nodeCatalogUnavailableDescription')) {
      offenders.add(`${path}: unavailable Catalog placeholder ${node.text}`);
    }
    if (ts.isObjectLiteralExpression(node)) {
      const reason = descriptorSynthesisReason(node);
      if (reason) offenders.add(`${path}: synthesized creation descriptor (${reason})`);
    }
    node.forEachChild(visit);
  }

  visit(sourceFile);
  for (const offender of descriptorAliasOffenders(path, sourceFile)) offenders.add(offender);
  return [...offenders];
}

function fixtureOffenders(source: string): string[] {
  return withIsolatedTypeScriptProject({ 'fixture.ts': source }, ({ sourceFile }) => (
    sourceOffenders('fixture.ts', sourceFile('fixture.ts'))
  ));
}


function containsCreateNodeMutation(sourceFile: ts.SourceFile): boolean {
  let found = false;
  function visit(node: ts.Node): void {
    if (ts.isPropertyAssignment(node)
      && propertyName(node.name) === 'type'
      && ts.isStringLiteral(node.initializer)
      && node.initializer.text === 'createNode') {
      found = true;
    }
    if (!found) node.forEachChild(visit);
  }
  visit(sourceFile);
  return found;
}

describe('scoped node Catalog architecture audit', () => {
  it.each([
    ['descriptor spread', 'const rebuilt = { ...descriptor };'],
    ['Catalog creation spread', 'const rebuilt = { ...item.creation, title };'],
    ['variable-kind reconstruction', 'const rebuilt = { kind, nodeTypeId: descriptor.nodeTypeId };'],
    [
      'descriptor-derived alias spread',
      'const source = template.descriptor; const rebuilt = { ...source };',
    ],
    [
      'descriptor-derived simple alias chain spread',
      'const source = template.descriptor; const alias = source; const rebuilt = { ...alias };',
    ],
    [
      'identity-bearing object alias spread',
      `const identity = {
        nodeTypeId: descriptor.nodeTypeId,
        resourcePath: descriptor.resourcePath,
        resourceRevision: descriptor.resourceRevision,
        createArgs: descriptor.createArgs,
      };
      const rebuilt = { kind, ...identity };`,
    ],
  ])('detects %s as descriptor synthesis', (_name, source) => {
    expect(fixtureOffenders(source)).toEqual([
      expect.stringContaining('synthesized creation descriptor'),
    ]);
  });

  it.each([
    'const payload = { descriptor };',
    'const payload = { descriptor: template.descriptor };',
    'const template = { title, descriptor };',
    'const source = template.descriptor; const payload = { descriptor: source };',
    'const positionCopy = { ...position };',
    `const source = template.descriptor;
     { const source = position; const positionCopy = { ...source }; }`,
  ])('allows exact forwarding negative control: %s', (source) => {
    expect(fixtureOffenders(source)).toEqual([]);
  });



  it('keeps Catalog, docs, creation, and DnD production paths descriptor-authoritative', () => {
    const auditedFiles = scopedDirectories
      .flatMap(productionFiles)
      .map((path) => relative(resolve('.'), resolve(path)).replace(/\\/g, '/'));
    const uniqueAuditedFiles = [...new Set(auditedFiles)].sort();
    const offenders = withProductionTypeScriptProject(({ sourceFile }) => (
      uniqueAuditedFiles.flatMap((path) => sourceOffenders(path, sourceFile(path)))
    ));

    expect(uniqueAuditedFiles.length).toBeGreaterThan(0);
    expect(offenders).toEqual([]);
  });

  it('keeps node creation behind one descriptor-authoritative mutation boundary', () => {
    const auditedFiles = scopedDirectories
      .flatMap(productionFiles)
      .map((path) => relative(resolve('.'), resolve(path)).replace(/\\/g, '/'));
    const uniqueAuditedFiles = [...new Set(auditedFiles)];
    const mutationSites = withProductionTypeScriptProject(({ sourceFile }) => (
      uniqueAuditedFiles.filter((path) => containsCreateNodeMutation(sourceFile(path)))
    ));

    expect(mutationSites).toHaveLength(1);
  });
});

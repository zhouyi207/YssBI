// @vitest-environment happy-dom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as ts from 'typescript';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalizedNodeCatalogState } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { getLocalizedSearchIndex } from '@/features/core/nodeCatalog/localizedSearchIndex';
import type { LocalizedCatalogResponse } from '@/features/core/nodeCatalog/nodeCatalogStore';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { NodePalette } from './NodePalette';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const catalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));

interface NamedModuleEdge {
  kind: 'import' | 'export';
  moduleSpecifier: string;
  symbol: string;
}

const productionPaletteChain = {
  'src/views/EditorView/Layout/NodePalette.tsx': [
    { kind: 'import', moduleSpecifier: '@/features/application/nodeCatalog/useLocalizedNodeCatalog', symbol: 'useLocalizedNodeCatalog' },
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/creationDescriptor', symbol: 'NodeCreationDescriptor' },
  ],
  'src/views/EditorView/Canvas/overlays/CanvasOverlays.tsx': [
    { kind: 'import', moduleSpecifier: '../../Layout/NodePalette', symbol: 'NodePalette' },
    { kind: 'import', moduleSpecifier: '@/features/application/editor', symbol: 'useCanvasOverlayHandlers' },
  ],
  'src/features/application/editor/index.ts': [
    { kind: 'export', moduleSpecifier: './useCanvasOverlayHandlers', symbol: 'useCanvasOverlayHandlers' },
  ],
  'src/features/application/editor/useCanvasOverlayHandlers.ts': [
    { kind: 'import', moduleSpecifier: '@/features/application/nodeCatalog/createNodeFromDescriptor', symbol: 'createNodeFromDescriptor' },
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/creationDescriptor', symbol: 'NodeCreationDescriptor' },
  ],
  'src/features/application/nodeCatalog/useLocalizedNodeCatalog.ts': [
    { kind: 'import', moduleSpecifier: '@/features/core/nodeCatalog/localizedSearchIndex', symbol: 'getLocalizedSearchIndex' },
    { kind: 'import', moduleSpecifier: '@/features/core/nodeCatalog/nodeCatalogStore', symbol: 'useNodeCatalogStore' },
    { kind: 'import', moduleSpecifier: '@/services/nodeSystem/catalogService', symbol: 'CatalogService' },
  ],
  'src/features/core/nodeCatalog/localizedSearchIndex.ts': [
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/search', symbol: 'searchLocalizedCatalogItems' },
    { kind: 'import', moduleSpecifier: './nodeCatalogStore', symbol: 'catalogResponseKey' },
  ],
  'src/features/core/nodeCatalog/nodeCatalogStore.ts': [
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/catalogItem', symbol: 'LocalizedCatalogItem' },
  ],
  'src/features/domain/nodeCatalog/search.ts': [
    { kind: 'import', moduleSpecifier: './catalogItem', symbol: 'LocalizedCatalogItem' },
  ],
  'src/features/domain/nodeCatalog/catalogItem.ts': [
    { kind: 'import', moduleSpecifier: './creationDescriptor', symbol: 'isNodeCreationDescriptor' },
    { kind: 'import', moduleSpecifier: './creationDescriptor', symbol: 'NodeCreationDescriptor' },
  ],
  'src/features/domain/nodeCatalog/creationDescriptor.ts': [],
  'src/features/application/nodeCatalog/createNodeFromDescriptor.ts': [
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/creationDescriptor', symbol: 'isNodeCreationDescriptor' },
    { kind: 'import', moduleSpecifier: '@/features/domain/nodeCatalog/creationDescriptor', symbol: 'NodeCreationDescriptor' },
    { kind: 'import', moduleSpecifier: '@/features/application/editorMutation/editorMutationCoordinator', symbol: 'executeEditorMutation' },
  ],
  'src/services/nodeSystem/catalogService.ts': [
    { kind: 'import', moduleSpecifier: '@tauri-apps/api/core', symbol: 'invoke' },
  ],
} as const satisfies Record<string, readonly NamedModuleEdge[]>;

function sourceFile(source: string): ts.SourceFile {
  return ts.createSourceFile('source-contract.tsx', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}

function importSymbols(statement: ts.ImportDeclaration): string[] {
  const clause = statement.importClause;
  if (!clause) return [];

  const symbols = clause.name ? ['default'] : [];
  if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
    symbols.push(...clause.namedBindings.elements.map((element) =>
      (element.propertyName ?? element.name).text));
  } else if (clause.namedBindings) {
    symbols.push('*');
  }
  return symbols;
}

function exportSymbols(statement: ts.ExportDeclaration): string[] {
  if (!statement.exportClause) return ['*'];
  if (ts.isNamedExports(statement.exportClause)) {
    return statement.exportClause.elements.map((element) =>
      (element.propertyName ?? element.name).text);
  }
  return ['*'];
}

function hasNamedModuleEdge(source: string, edge: NamedModuleEdge): boolean {
  return sourceFile(source).statements.some((statement) => {
    if (edge.kind === 'import' && ts.isImportDeclaration(statement)) {
      return ts.isStringLiteral(statement.moduleSpecifier)
        && statement.moduleSpecifier.text === edge.moduleSpecifier
        && importSymbols(statement).includes(edge.symbol);
    }
    if (edge.kind === 'export' && ts.isExportDeclaration(statement)) {
      return statement.moduleSpecifier !== undefined
        && ts.isStringLiteral(statement.moduleSpecifier)
        && statement.moduleSpecifier.text === edge.moduleSpecifier
        && exportSymbols(statement).includes(edge.symbol);
    }
    return false;
  });
}

function containsIdentifier(source: string, identifier: string): boolean {
  let found = false;
  function visit(node: ts.Node): void {
    if (ts.isIdentifier(node) && node.text === identifier) found = true;
    if (!found) ts.forEachChild(node, visit);
  }
  visit(sourceFile(source));
  return found;
}

function hasImportFromModule(source: string, moduleSpecifier: string): boolean {
  return sourceFile(source).statements.some((statement) =>
    ts.isImportDeclaration(statement)
    && ts.isStringLiteral(statement.moduleSpecifier)
    && statement.moduleSpecifier.text === moduleSpecifier);
}

function expressionHasName(expression: ts.Expression, name: string): boolean {
  if (ts.isIdentifier(expression)) return expression.text === name;
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text === name;
  return ts.isElementAccessExpression(expression)
    && ts.isStringLiteral(expression.argumentExpression)
    && expression.argumentExpression.text === name;
}

function containsCall(source: string, callee: string): boolean {
  let found = false;
  function visit(node: ts.Node): void {
    if (ts.isCallExpression(node) && expressionHasName(node.expression, callee)) found = true;
    if (!found) ts.forEachChild(node, visit);
  }
  visit(sourceFile(source));
  return found;
}

const legacyPaletteSymbols = [
  'NodeDefinition',
  'buildBuiltinCatalogItems',
  'buildContextualCatalogItems',
  'filterCatalogItems',
  'buildNodeTemplateDragData',
  'resolveEffectiveDefinition',
  'isNodeCompatibleWithPin',
  'pinAcceptsType',
  'pinCompatibility',
] as const;

const legacyPaletteModuleNames = new Set([
  'pinCompatibility',
  'buildBuiltinCatalogItems',
  'buildContextualCatalogItems',
  'filterCatalogItems',
  'buildNodeTemplateDragData',
]);

function importedModuleNames(source: string): string[] {
  return sourceFile(source).statements.flatMap((statement) => {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) return [];
    const segments = statement.moduleSpecifier.text.split('/');
    const basename = segments[segments.length - 1] ?? '';
    return [basename.replace(/\.[^.]+$/, '')];
  });
}

function hasForbiddenLegacyDependency(source: string): boolean {
  return legacyPaletteSymbols.some((symbol) => containsIdentifier(source, symbol))
    || importedModuleNames(source).some((moduleName) => legacyPaletteModuleNames.has(moduleName));
}

function hasDisallowedTauriAccess(source: string): boolean {
  return hasImportFromModule(source, '@tauri-apps/api/core') || containsCall(source, 'invoke');
}

vi.mock('@/features/application/nodeCatalog/useLocalizedNodeCatalog', () => ({
  useLocalizedNodeCatalog: () => catalogState.current,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => ({
      'common.loading': 'Loading...',
      'common.error': 'Error',
      'canvas.nodePalette.searchPlaceholder': 'Search nodes...',
      'canvas.nodePalette.noMatches': 'No matches found',
    }[key] ?? key),
  }),
}));

const catalog: LocalizedCatalogResponse = {
  projectInstanceId: 'project-1',
  registryFingerprint: 'registry-1',
  resourcePublicationRevision: 7,
  locale: 'zh-CN',
  categories: [
    { categoryId: 'math', title: '数学', searchText: '数学 math' },
    { categoryId: 'output', title: '输出', searchText: '输出 output' },
  ],
  items: [
    {
      nodeTypeId: 'math.add',
      title: '加法',
      description: '将两个数字相加',
      documentation: null,
      categoryId: 'math',
      aliases: ['sum'],
      technicalTerms: ['addition'],
      pinyin: 'jia fa',
      creation: { kind: 'static', nodeTypeId: 'math.add' },
      searchText: '加法 数学 sum addition jia fa',
    },
    {
      nodeTypeId: 'output.print',
      title: '打印',
      description: null,
      documentation: null,
      categoryId: 'output',
      aliases: ['print'],
      technicalTerms: [],
      pinyin: 'da yin',
      creation: { kind: 'static', nodeTypeId: 'output.print' },
      searchText: '打印 输出 print da yin',
    },
  ],
};

function readyState(): LocalizedNodeCatalogState {
  return {
    status: 'ready',
    error: null,
    catalog,
    searchIndex: getLocalizedSearchIndex(catalog),
  };
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('NodePalette', () => {
  it('does not accept comments, strings, or unrelated imports as production edges', () => {
    const decoySource = `
      // import { useCanvasOverlayHandlers } from '@/features/application/editor';
      const documentation = "import { useCanvasOverlayHandlers } from '@/features/application/editor'";
      import { useEditorGroup } from '@/features/application/editor';
    `;

    expect(hasNamedModuleEdge(decoySource, {
      kind: 'import',
      moduleSpecifier: '@/features/application/editor',
      symbol: 'useCanvasOverlayHandlers',
    })).toBe(false);
  });

  it.each([
    { name: 'named', source: "import { invoke } from '@tauri-apps/api/core';" },
    { name: 'aliased named', source: "import { invoke as callTauri } from '@tauri-apps/api/core';" },
    { name: 'namespace', source: "import * as tauriCore from '@tauri-apps/api/core';" },
    { name: 'default', source: "import tauriCore from '@tauri-apps/api/core';" },
    { name: 'side-effect', source: "import '@tauri-apps/api/core';" },
  ])('rejects forbidden Tauri $name imports independently of calls', ({ source }) => {
    expect(hasDisallowedTauriAccess(source)).toBe(true);
  });

  it('detects an import-free property-access invoke call', () => {
    expect(hasDisallowedTauriAccess("tauriCore.invoke('command');")).toBe(true);
  });

  it('detects an import-free bracket-access invoke call', () => {
    expect(hasDisallowedTauriAccess("tauriCore['invoke']('command');")).toBe(true);
  });

  it('allows an import-free benign property call', () => {
    expect(hasDisallowedTauriAccess("tauriCore.listen('event');")).toBe(false);
  });

  it('rejects renamed compatibility imports by module specifier', () => {
    const renamedCompatibilitySource = `
      import { isPinCompatible } from '@/shared/utils/pinCompatibility';
      void isPinCompatible;
    `;

    expect(hasForbiddenLegacyDependency(renamedCompatibilitySource)).toBe(true);
  });

  it('keeps the complete production static palette chain on Catalog descriptors', () => {
    const sources = Object.entries(productionPaletteChain).map(([path, requiredEdges]) => ({
      path,
      requiredEdges,
      source: readFileSync(resolve(path), 'utf8'),
    }));

    const missingChainLinks = sources.flatMap(({ path, requiredEdges, source }) =>
      requiredEdges
        .filter((edge) => !hasNamedModuleEdge(source, edge))
        .map((edge) => `${path}: ${edge.kind} { ${edge.symbol} } from '${edge.moduleSpecifier}'`));
    const legacyOffenders = sources
      .filter(({ source }) => hasForbiddenLegacyDependency(source))
      .map(({ path }) => path);
    const directInvokeOffenders = sources
      .filter(({ path }) => path !== 'src/services/nodeSystem/catalogService.ts')
      .filter(({ source }) => hasDisallowedTauriAccess(source))
      .map(({ path }) => path);

    expect(missingChainLinks).toEqual([]);
    expect(legacyOffenders).toEqual([]);
    expect(directInvokeOffenders).toEqual([]);
  });

  let host: HTMLDivElement;
  let root: Root;
  const onSelect = vi.fn<(descriptor: NodeCreationDescriptor, locale: string) => void>();

  beforeEach(() => {
    vi.clearAllMocks();
    catalogState.current = readyState();
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderPalette(): void {
    act(() => root.render(<NodePalette x={12} y={34} onSelect={onSelect} />));
  }

  it('renders a loading state while the localized catalog is loading', () => {
    catalogState.current = { status: 'loading', error: null, catalog: null, searchIndex: null };

    renderPalette();

    expect(host.textContent).toContain('Loading...');
  });

  it('renders the catalog load error', () => {
    catalogState.current = {
      status: 'error',
      error: 'Catalog request failed',
      catalog: null,
      searchIndex: null,
    };

    renderPalette();

    expect(host.textContent).toContain('Catalog request failed');
  });

  it('renders localized categories and items from the catalog response', () => {
    renderPalette();

    expect(host.textContent).toContain('数学');
    expect(host.textContent).toContain('加法');
    expect(host.textContent).toContain('输出');
    expect(host.textContent).toContain('打印');
  });

  it('filters rendered items through the localized search index', () => {
    renderPalette();
    const input = host.querySelector('input');
    expect(input).not.toBeNull();

    act(() => setInputValue(input!, 'sum'));

    expect(host.textContent).toContain('加法');
    expect(host.textContent).not.toContain('打印');
  });

  it('renders an empty state when search has no matches', () => {
    renderPalette();
    const input = host.querySelector('input');

    act(() => setInputValue(input!, 'missing node'));

    expect(host.textContent).toContain('No matches found');
  });

  it('selects the Rust-issued descriptor with its catalog locale', () => {
    renderPalette();
    const item = Array.from(host.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('加法'));
    expect(item).toBeDefined();

    act(() => item!.click());

    expect(onSelect).toHaveBeenCalledWith(
      { kind: 'static', nodeTypeId: 'math.add' },
      'zh-CN',
    );
  });
});

// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalizedNodeCatalogState } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { getLocalizedSearchIndex } from '@/features/core/nodeCatalog/localizedSearchIndex';
import type { LocalizedCatalogResponse } from '@/features/core/nodeCatalog/nodeCatalogStore';
import { useNodeCatalogTreeStore } from '@/features/core/nodeCatalog/nodeCatalogTreeStore';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { TooltipProvider } from '@/components/ui/tooltip';
import { NodePalette } from './NodePalette';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const catalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));
const compatibleCatalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));

vi.mock('@/features/application/nodeCatalog/useLocalizedNodeCatalog', () => ({
  useLocalizedNodeCatalog: () => catalogState.current,
}));

vi.mock('@/features/application/nodeCatalog/useCompatibleNodeCatalog', () => ({
  useCompatibleNodeCatalog: () => compatibleCatalogState.current,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => ({
      'common.loading': 'Loading...',
      'common.error': 'Error',
      'common.incidentId': 'Incident ID',
      'nodeCatalog.loadError': 'Node catalog unavailable',
      'canvas.nodePalette.searchPlaceholder': 'Search nodes...',
      'canvas.nodePalette.collapseAll': 'Collapse All',
      'canvas.nodePalette.expandAll': 'Expand All',
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
    { categoryId: 'statistics.regression', parentCategoryId: 'statistics', order: 11, title: '回归', searchText: '回归 regression' },
    { categoryId: 'math', parentCategoryId: null, order: 20, title: '数学', searchText: '数学 math' },
    { categoryId: 'output', parentCategoryId: null, order: 30, title: '输出', searchText: '输出 output' },
    { categoryId: 'functions', parentCategoryId: null, order: 40, title: '函数', searchText: '函数 functions' },
    { categoryId: 'statistics', parentCategoryId: null, order: 10, title: '统计', searchText: '统计 statistics' },
  ],
  items: [
    {
      nodeTypeId: 'statistics.logit.fit',
      title: '逻辑回归',
      description: '拟合二元响应模型',
      documentation: null,
      categoryId: 'statistics.regression',
      iconId: 'statistics',
      styleId: 'default',
      aliases: ['logit'],
      technicalTerms: ['logistic regression'],
      backendSearchText: [],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId: 'statistics.logit.fit' },
    },
    {
      nodeTypeId: 'math.add',
      title: '加法',
      description: '将两个数字相加',
      documentation: null,
      categoryId: 'math',
      iconId: 'math',
      styleId: 'default',
      aliases: ['sum'],
      technicalTerms: ['addition', '加法术语'],
      backendSearchText: ['backend-add-token'],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId: 'math.add' },
    },
    {
      nodeTypeId: 'output.print',
      title: '打印',
      description: null,
      documentation: null,
      categoryId: 'output',
      iconId: 'output',
      styleId: 'default',
      aliases: ['print'],
      technicalTerms: [],
      backendSearchText: [],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId: 'output.print' },
    },
    {
      nodeTypeId: 'function.call',
      title: '调用 Helper',
      description: null,
      documentation: null,
      categoryId: 'functions',
      iconId: 'function',
      styleId: 'call',
      aliases: ['Helper'],
      technicalTerms: [],
      backendSearchText: ['invoke helper'],
      resourceNames: ['Helper Resource'],
      ports: [],
      parameters: [],
      resourcePath: 'functions/Helper.yssbi-function',
      resourceRevision: 3,
      creation: {
        kind: 'resourceBound',
        nodeTypeId: 'function.call',
        resourcePath: 'functions/Helper.yssbi-function',
        resourceRevision: 3,
        createArgs: { kind: 'function' },
      },
    },
  ],
};

function readyState(): LocalizedNodeCatalogState {
  return {
    status: 'ready',
    error: null,
    catalog,
    searchIndex: getLocalizedSearchIndex(catalog),
    refresh: vi.fn(),
  };
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('NodePalette', () => {

  let host: HTMLDivElement;
  let root: Root;
  const onSelect = vi.fn<(descriptor: NodeCreationDescriptor, locale: string) => void>();
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useNodeCatalogTreeStore.getState().reset();
    catalogState.current = readyState();
    compatibleCatalogState.current = {
      status: 'idle', error: null, catalog: null, searchIndex: null, refresh: vi.fn(),
    };
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderPalette(): void {
    act(() => root.render(
      <TooltipProvider>
        <NodePalette x={12} y={34} onSelect={onSelect} onClose={onClose} />
      </TooltipProvider>,
    ));
  }

  it('dismisses when pointerdown happens outside the palette', () => {
    const outside = document.createElement('button');
    document.body.appendChild(outside);
    renderPalette();

    act(() => outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));

    expect(onClose).toHaveBeenCalledOnce();
    outside.remove();
  });

  it('toggles all categories and disables the toggle while searching', () => {
    renderPalette();

    expect(host.querySelector('[data-sidebar-tree-search]')).not.toBeNull();
    const toggle = host.querySelector<HTMLButtonElement>('[data-sidebar-tree-expand-toggle]');
    const statistics = host.querySelector<HTMLButtonElement>(
      '[data-catalog-category-id="statistics"]',
    );
    expect(toggle?.disabled).toBe(false);
    expect(statistics?.disabled).toBe(false);
    expect(host.textContent).toContain('逻辑回归');
    expect(toggle?.getAttribute('aria-label')).toBe('Collapse All');

    act(() => toggle?.click());
    expect(host.textContent).not.toContain('逻辑回归');
    expect(host.textContent).not.toContain('加法');
    expect(host.textContent).not.toContain('打印');
    expect(toggle?.getAttribute('aria-label')).toBe('Expand All');

    act(() => toggle?.click());
    expect(host.textContent).toContain('逻辑回归');
    expect(host.textContent).toContain('加法');
    expect(host.textContent).toContain('打印');
    expect(toggle?.getAttribute('aria-label')).toBe('Collapse All');

    const input = host.querySelector('input')!;
    act(() => setInputValue(input, 'logit'));
    expect(toggle?.disabled).toBe(true);
    expect(statistics?.disabled).toBe(true);
    expect(statistics?.getAttribute('aria-disabled')).toBe('true');
    expect(host.textContent).toContain('逻辑回归');
  });

  it('uses only the backend-compatible catalog for an edge-drop palette', () => {
    const compatibleCatalog = {
      ...catalog,
      items: [catalog.items.find((item) => item.nodeTypeId === 'output.print')!],
      categories: [catalog.categories.find((category) => category.categoryId === 'output')!],
    };
    compatibleCatalogState.current = {
      status: 'ready',
      error: null,
      catalog: compatibleCatalog,
      searchIndex: getLocalizedSearchIndex(compatibleCatalog),
      refresh: vi.fn(),
    };

    act(() => root.render(
      <TooltipProvider>
        <NodePalette
          x={12}
          y={34}
          graphPath="events/Main.yssbi-event"
          graphRevision={7}
          sourcePort={{
            kind: 'declared',
            nodeId: '00000000-0000-0000-0000-000000000101',
            portKey: 'value',
          }}
          onSelect={onSelect}
        />
      </TooltipProvider>,
    ));

    expect(host.textContent).toContain('打印');
    expect(host.textContent).not.toContain('加法');
    const input = host.querySelector('input')!;
    act(() => setInputValue(input, 'print'));
    expect(host.textContent).toContain('打印');
  });

  it('renders a loading state while the localized catalog is loading', () => {
    catalogState.current = {
      status: 'loading', error: null, catalog: null, searchIndex: null, refresh: vi.fn(),
    };

    renderPalette();

    expect(host.textContent).toContain('Loading...');
  });

  it('renders localized generic catalog text, code, and incident ID', () => {
    catalogState.current = {
      status: 'error',
      error: {
        code: 'catalog_backend_failed',
        incidentId: 'incident-catalog-42',
      },
      catalog: null,
      searchIndex: null,
      refresh: vi.fn(),
    };

    renderPalette();

    expect(host.textContent).toContain('Node catalog unavailable');
    expect(host.textContent).toContain('[catalog_backend_failed]');
    expect(host.textContent).toContain('Incident ID: incident-catalog-42');
  });

  it('keeps rendering the last catalog when a refresh fails', () => {
    catalogState.current = {
      ...readyState(),
      status: 'error',
      error: { code: 'catalog_refresh_failed', incidentId: null },
    };

    renderPalette();

    expect(host.textContent).toContain('加法');
    expect(host.textContent).not.toContain('catalog_refresh_failed');
  });

  it('renders localized categories and items from the catalog response', () => {
    renderPalette();

    expect(host.textContent).toContain('数学');
    expect(host.textContent).toContain('加法');
    expect(host.textContent).toContain('输出');
    expect(host.textContent).toContain('打印');
  });

  it('renders categories as controlled Collapsible rows and expands search ancestors', () => {
    renderPalette();

    const statistics = host.querySelector<HTMLButtonElement>(
      'button[data-catalog-category-id="statistics"]',
    );
    expect(statistics).not.toBeNull();
    expect(statistics?.getAttribute('aria-expanded')).toBe('true');

    const input = host.querySelector('input');
    act(() => setInputValue(input!, 'logit'));

    const regression = host.querySelector<HTMLButtonElement>(
      'button[data-catalog-category-id="statistics.regression"]',
    );
    expect(regression?.getAttribute('aria-expanded')).toBe('true');
  });

  it('renders child categories beneath their parent in declared order', () => {
    renderPalette();

    const categories = Array.from(host.querySelectorAll('[data-catalog-category-id]'));
    expect(categories.map((element) => element.getAttribute('data-catalog-category-id'))).toEqual([
      'statistics',
      'statistics.regression',
      'math',
      'output',
      'functions',
    ]);
    const regression = host.querySelector('[data-catalog-category-id="statistics.regression"]');
    expect(regression?.getAttribute('data-catalog-depth')).toBe('1');
    expect(host.querySelector('[data-catalog-item-key="item:static:statistics.logit.fit"]')?.textContent)
      .toContain('逻辑回归');
  });

  it.each([
    ['localized title', '加法'],
    ['alias', 'sum'],
    ['technical term', 'addition'],
    ['technical term full pinyin', 'jia fa shu yu'],
    ['technical term pinyin initials', 'jfsy'],
    ['stable node ID', 'math.add'],
    ['backend search text', 'backend-add-token'],
    ['full pinyin', 'jia fa'],
    ['pinyin initials', 'jf'],
  ])('filters rendered items through %s', (_field, query) => {
    renderPalette();
    const input = host.querySelector('input');
    expect(input).not.toBeNull();

    act(() => setInputValue(input!, query));
    expect(host.textContent).toContain('加法');
    expect(host.textContent).not.toContain('打印');
  });

  it('filters resource entries through authoritative resource names', () => {
    renderPalette();
    const input = host.querySelector('input')!;

    act(() => setInputValue(input, 'helper resource'));

    expect(host.textContent).toContain('调用 Helper');
    expect(host.textContent).not.toContain('加法');
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

  it('keeps same-type resources distinct across search, refresh, and locale changes', () => {
    const first = catalog.items.find((item) => item.nodeTypeId === 'function.call')!;
    const second = {
      ...first,
      title: '调用 Other',
      aliases: ['other-only'],
      resourcePath: 'functions/Other.yssbi-function',
      creation: {
        ...first.creation,
        resourcePath: 'functions/Other.yssbi-function',
      } as NodeCreationDescriptor,
    };
    const multiCatalog = {
      ...catalog,
      resourcePublicationRevision: 70,
      items: [...catalog.items, second],
    };
    catalogState.current = {
      status: 'ready', error: null, catalog: multiCatalog,
      searchIndex: getLocalizedSearchIndex(multiCatalog), refresh: vi.fn(),
    };
    renderPalette();
    const input = host.querySelector('input')!;
    act(() => setInputValue(input, 'other-only'));
    expect(host.textContent).toContain('调用 Other');
    expect(host.textContent).not.toContain('调用 Helper');

    const refreshed = {
      ...multiCatalog,
      resourcePublicationRevision: 71,
      items: multiCatalog.items.map((item) => item.resourcePath === second.resourcePath
        ? { ...item, title: '刷新 Other' }
        : item),
    };
    catalogState.current = {
      status: 'ready', error: null, catalog: refreshed,
      searchIndex: getLocalizedSearchIndex(refreshed), refresh: vi.fn(),
    };
    act(() => root.render(
      <TooltipProvider>
        <NodePalette x={12} y={34} onSelect={onSelect} />
      </TooltipProvider>,
    ));
    expect(host.textContent).toContain('刷新 Other');

    const localized = {
      ...refreshed,
      locale: 'en-US',
      items: refreshed.items.map((item) => item.resourcePath === second.resourcePath
        ? { ...item, title: 'Call Other' }
        : item),
    };
    catalogState.current = {
      status: 'ready', error: null, catalog: localized,
      searchIndex: getLocalizedSearchIndex(localized), refresh: vi.fn(),
    };
    act(() => root.render(
      <TooltipProvider>
        <NodePalette x={12} y={34} onSelect={onSelect} />
      </TooltipProvider>,
    ));
    expect(host.textContent).toContain('Call Other');
    expect(host.querySelectorAll('button[data-catalog-item-key]')).toHaveLength(1);
  });

  it('selects a resource descriptor without reconstructing its opaque identity', () => {
    renderPalette();
    const item = Array.from(host.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('调用 Helper'));

    act(() => item!.click());

    expect(onSelect).toHaveBeenCalledWith({
      kind: 'resourceBound',
      nodeTypeId: 'function.call',
      resourcePath: 'functions/Helper.yssbi-function',
      resourceRevision: 3,
      createArgs: { kind: 'function' },
    }, 'zh-CN');
  });
});

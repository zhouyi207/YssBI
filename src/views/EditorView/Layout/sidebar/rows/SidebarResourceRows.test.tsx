// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalizedNodeCatalogState } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { SidebarDataRow } from './SidebarDataRow';
import { SidebarVariableRow } from './SidebarVariableRow';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  catalogState: null as LocalizedNodeCatalogState | null,
  draggableInputs: [] as Array<{ data: unknown; disabled?: boolean }>,
  dragPointerDown: vi.fn(),
}));

vi.mock('@dnd-kit/core', () => ({
  useDraggable: (input: { data: unknown; disabled?: boolean }) => {
    mocks.draggableInputs.push(input);
    return {
      attributes: {},
      listeners: { onPointerDown: mocks.dragPointerDown },
      setNodeRef: vi.fn(),
    };
  },
}));
vi.mock('@/features/application/nodeCatalog/useLocalizedNodeCatalog', () => ({
  useLocalizedNodeCatalog: () => mocks.catalogState,
}));
vi.mock('@/features/application/editor', () => ({ focusDetails: vi.fn() }));
vi.mock('@/features/application/window', () => ({ openDatabaseEditorWindow: vi.fn() }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
  TooltipContent: ({ children }: { children: React.ReactNode }) => children,
}));

const variablePath = 'variables/00000000-0000-0000-0000-000000000001';
const databasePath = 'databases/sales / . # 数据';
const variableGet: NodeCreationDescriptor = {
  kind: 'resourceBound',
  nodeTypeId: 'yssbi.project.variable.get',
  resourcePath: variablePath,
  resourceRevision: 3,
  createArgs: { kind: 'variable' },
};

const databaseSource: NodeCreationDescriptor = {
  kind: 'resourceBound',
  nodeTypeId: 'yssbi.dataframe.source.get',
  resourcePath: databasePath,
  resourceRevision: 4,
  createArgs: { kind: 'database' },
};

function item(title: string, descriptor: NodeCreationDescriptor) {
  return {
    nodeTypeId: descriptor.nodeTypeId,
    title,
    description: null,
    documentation: null,
    categoryId: 'resources',
    iconId: 'resource',
    styleId: 'default',
    aliases: [],
    technicalTerms: [],
    backendSearchText: [title],
    resourceNames: [title],
    ports: [],
    parameters: [],
    resourcePath: descriptor.kind === 'resourceBound' ? descriptor.resourcePath : undefined,
    resourceRevision: descriptor.kind === 'resourceBound' ? descriptor.resourceRevision : undefined,
    creation: descriptor,
  };
}

function catalogState(
  status: LocalizedNodeCatalogState['status'] = 'ready',
  items = [item('Get Counter', variableGet), item('Sales', databaseSource)],
): LocalizedNodeCatalogState {
  return {
    status,
    error: status === 'error'
      ? { code: 'catalog_response_stale', incidentId: null }
      : null,
    catalog: {
      projectInstanceId: 'project-1',
      registryFingerprint: 'registry-1',
      resourcePublicationRevision: 8,
      locale: 'en-US',
      categories: [],
      items,
    },
    searchIndex: null,
    refresh: vi.fn(),
  };
}

describe('resource sidebar rows', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.draggableInputs.length = 0;
    mocks.catalogState = catalogState();
    useVariableStore.getState().clear();
    useDatabaseStore.getState().clear();
    useProjectIOStore.setState({ refreshResourceIndex: vi.fn().mockResolvedValue(true) });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderVariable(resourcePath: string | null = variablePath, isGlobal = true) {
    act(() => root.render(
      <SidebarVariableRow
        id="variable-id"
        resourcePath={resourcePath ?? undefined}
        name="Counter"
        dataType={{ kind: 'Int64' }}
        isGlobal={isGlobal}
        onContextMenu={vi.fn()}
      />,
    ));
  }

  function renderDatabase(
    resourcePath: string | null = databasePath,
    data: unknown = {},
  ) {
    act(() => root.render(
      <SidebarDataRow
        id="database-id"
        resourcePath={resourcePath ?? undefined}
        name="Sales"
        data={data}
        onContextMenu={vi.fn()}
      />,
    ));
  }

  it.each([
    ['global', true],
    ['local', false],
  ])('uses the exact current variable Get descriptor for a %s variable', (_scope, isGlobal) => {
    renderVariable(variablePath, isGlobal);

    const input = mocks.draggableInputs[mocks.draggableInputs.length - 1];
    expect(input).toMatchObject({
      disabled: false,
      data: { type: 'node-template', template: { title: 'Counter', descriptor: variableGet } },
    });
    const dragData = input?.data as { template?: { descriptor?: unknown } };
    expect(dragData.template?.descriptor).toBe(variableGet);
  });

  it('forwards pointer down to the dnd-kit drag listener', () => {
    renderVariable();
    const row = host.firstElementChild;

    act(() => row?.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));

    expect(mocks.dragPointerDown).toHaveBeenCalledOnce();
  });

  it('uses the exact current database source descriptor', () => {
    renderDatabase();

    const input = mocks.draggableInputs[mocks.draggableInputs.length - 1];
    expect(input).toMatchObject({
      disabled: false,
      data: { type: 'node-template', template: { title: 'Sales', descriptor: databaseSource } },
    });
    const dragData = input?.data as { template?: { descriptor?: unknown } };
    expect(dragData.template?.descriptor).toBe(databaseSource);
  });

  it('shows the localized load failure tooltip from machine state', () => {
    renderDatabase(databasePath, { loadFailed: true });

    expect(host.textContent).toContain('sidebar.dataLoadFailed');
  });

  it('does not treat a legacy raw load error as failure state', () => {
    renderDatabase(databasePath, { loadError: 'sensitive backend failure' });

    expect(host.textContent).not.toContain('sidebar.dataLoadFailed');
    expect(host.textContent).not.toContain('sensitive backend failure');
  });

  it.each([
    ['stale', catalogState('loading')],
    ['missing', catalogState('ready', [])],
  ])('disables a variable row for a %s descriptor and refreshes on interaction', (_case, state) => {
    mocks.catalogState = state;
    renderVariable();
    const row = host.querySelector('[aria-disabled="true"]');

    expect(row).not.toBeNull();
    expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]?.disabled).toBe(true);
    act(() => row!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));

    expect(useProjectIOStore.getState().refreshResourceIndex).not.toHaveBeenCalled();
    expect(state.refresh).toHaveBeenCalledOnce();
  });

  it('disables a database row when its exact descriptor is missing', () => {
    const state = catalogState('ready', [item('Other database', {
      ...databaseSource,
      resourcePath: 'databases/other',
    })]);
    mocks.catalogState = state;
    renderDatabase();
    const row = host.querySelector('[aria-disabled="true"]');

    expect(row).not.toBeNull();
    act(() => row!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));
    expect(useProjectIOStore.getState().refreshResourceIndex).not.toHaveBeenCalled();
    expect(state.refresh).toHaveBeenCalledOnce();
  });

  it('hydrates a missing variable path through ProjectIndex before refreshing Catalog and dragging', async () => {
    const state = catalogState('ready', []);
    mocks.catalogState = state;
    const refreshResourceIndex = vi.fn(async () => {
      useVariableStore.setState({
        variables: {
          'variable-id': {
            id: 'variable-id',
            resourcePath: variablePath,
            name: 'Counter',
            dataType: { kind: 'Int64' },
            dataValue: { kind: 'Int64', value: 1 },
            description: '',
            scope: { type: 'global' },
            tags: [],
          },
        },
        revisions: { 'variable-id': 1 },
      });
      return true;
    });
    useProjectIOStore.setState({ refreshResourceIndex });
    renderVariable(null);
    const row = host.querySelector('[aria-disabled="true"]');

    await act(async () => {
      row!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
      await Promise.resolve();
    });

    expect(refreshResourceIndex).toHaveBeenCalledOnce();
    expect(state.refresh).toHaveBeenCalledOnce();
    mocks.catalogState = catalogState('ready', [item('Get Counter', variableGet)]);
    renderVariable(useVariableStore.getState().variables['variable-id']?.resourcePath);
    expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]).toMatchObject({
      disabled: false,
      data: { type: 'node-template', template: { title: 'Counter', descriptor: variableGet } },
    });
  });

  it('suppresses Catalog refresh when missing-path ProjectIndex hydration becomes stale', async () => {
    const state = catalogState('ready', []);
    mocks.catalogState = state;
    const refreshResourceIndex = vi.fn().mockResolvedValue(false);
    useProjectIOStore.setState({ refreshResourceIndex });
    renderVariable(null);
    const row = host.querySelector('[aria-disabled="true"]');

    await act(async () => {
      row!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
      await Promise.resolve();
    });

    expect(refreshResourceIndex).toHaveBeenCalledOnce();
    expect(state.refresh).not.toHaveBeenCalled();
  });

  it('hydrates a missing database path through ProjectIndex before refreshing Catalog and dragging', async () => {
    const state = catalogState('loading', []);
    mocks.catalogState = state;
    const refreshResourceIndex = vi.fn(async () => {
      useDatabaseStore.setState({
        databases: {
          'database-id': { id: 'database-id', name: 'Sales', resourcePath: databasePath },
        },
      });
      return true;
    });
    useProjectIOStore.setState({ refreshResourceIndex });
    renderDatabase(null);
    const row = host.querySelector('[aria-disabled="true"]');

    await act(async () => {
      row!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
      await Promise.resolve();
    });

    expect(refreshResourceIndex).toHaveBeenCalledOnce();
    expect(state.refresh).toHaveBeenCalledOnce();
    mocks.catalogState = catalogState('ready', [item('Sales', databaseSource)]);
    renderDatabase(useDatabaseStore.getState().databases['database-id']?.resourcePath);
    expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]).toMatchObject({
      disabled: false,
      data: { type: 'node-template', template: { title: 'Sales', descriptor: databaseSource } },
    });
  });
});

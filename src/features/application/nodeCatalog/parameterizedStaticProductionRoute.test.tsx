// @vitest-environment happy-dom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { i18n } from '@/app/i18n';
import { validateEditorGraphProjection } from '@/features/domain/editorProjection';
import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  useGraphDataStore,
} from '@/features/core/dataStore/graphDataStore';
import {
  executeEditorMutation,
  resetEditorMutationCoordinator,
} from '@/features/application/editorMutation/editorMutationCoordinator';
import { CatalogService } from '@/services/nodeSystem/catalogService';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { isLocalizedCatalogDto } from '@/shared/types/dto/localizedCatalog';
import { NodeParameterEditor } from '@/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor';
import { createNodeFromDescriptor } from './createNodeFromDescriptor';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));


interface RouteStep {
  locale: string;
  operationId: string;
  mutation: EditorGraphMutationDto;
  result: GraphMutationResultDto;
}

interface ProductionRouteFixture {
  catalog: unknown;
  graphPath: string;
  projectNodeId: string;
  initialProjection: EditorGraphProjectionDto;
  create: RouteStep & { position: { x: number; y: number } };
  connect: RouteStep;
  submit: RouteStep & { selectedColumns: string[] };
}

const fixture = JSON.parse(readFileSync(
  resolve('src/tests/fixtures/parameterized-static-production-route.json'),
  'utf8',
)) as ProductionRouteFixture;

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

function click(element: Element | null): void {
  if (!element) throw new Error('missing test element');
  element.dispatchEvent(new MouseEvent('click', { bubbles: true }));
}

beforeEach(async () => {
  vi.clearAllMocks();
  await i18n.changeLanguage('en-US');
  resetEditorMutationCoordinator();
  useGraphDataStore.setState({ graphEntities: {} });
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.restoreAllMocks();
});

describe('ParameterizedStatic production route', () => {
  it('uses one Rust-authoritative fixture from Catalog descriptor through ordered editor submit', async () => {
    expect(isLocalizedCatalogDto(fixture.catalog)).toBe(true);
    if (!isLocalizedCatalogDto(fixture.catalog)) throw new Error('invalid Rust Catalog fixture');
    const initialProjection = validateEditorGraphProjection(fixture.initialProjection);
    const prepared = prepareGraphProjectionReplacements([{
      graphPath: fixture.graphPath,
      projection: initialProjection,
    }]);
    if (!prepared.prepared) throw new Error(`invalid Rust projection fixture: ${prepared.reason}`);
    commitPreparedGraphProjectionReplacements(prepared.plan);

    vi.mocked(invoke)
      .mockResolvedValueOnce(fixture.catalog)
      .mockResolvedValueOnce(fixture.create.result)
      .mockResolvedValueOnce(fixture.connect.result)
      .mockResolvedValueOnce(fixture.submit.result);
    const operationIds = [
      fixture.create.operationId,
      fixture.connect.operationId,
      fixture.submit.operationId,
    ];
    vi.spyOn(globalThis.crypto, 'randomUUID').mockImplementation(
      () => operationIds.shift() as `${string}-${string}-${string}-${string}-${string}`,
    );

    const catalog = await CatalogService.getLocalizedCatalog(
      fixture.catalog.projectInstanceId,
      fixture.catalog.locale,
    );
    const projectItem = catalog.items.find(
      (item) => item.nodeTypeId === 'yssbi.dataframe.project',
    );
    if (!projectItem) throw new Error('Rust Catalog omitted Project DataFrame');

    await createNodeFromDescriptor({
      graphPath: fixture.graphPath,
      locale: fixture.create.locale,
      descriptor: projectItem.creation,
      position: fixture.create.position,
    });

    const createOperations = fixture.create.result.delta.payload.operations as Array<{
      operation: string;
      node?: { id: string };
    }>;
    const createdNodeId = createOperations.find(
      (operation) => operation.operation === 'insert_node',
    )?.node?.id;
    if (!createdNodeId) throw new Error('Rust create result omitted the new node ID');
    expect(createdNodeId).toBe(fixture.projectNodeId);
    expect(fixture.submit.mutation).toMatchObject({
      type: 'setParameters',
      payload: { nodeId: createdNodeId },
    });

    let mutationCalls = vi.mocked(invoke).mock.calls.filter(([command]) =>
      command === 'mutate_graph_document');
    expect(mutationCalls[0]).toEqual([
      'mutate_graph_document',
      {
        graphPath: fixture.graphPath,
        locale: fixture.create.locale,
        request: {
          resource: { kind: 'graph', key: fixture.graphPath },
          baseRevision: 1,
          operationId: fixture.create.operationId,
          payload: fixture.create.mutation,
        },
      },
    ]);

    await executeEditorMutation({
      graphPath: fixture.graphPath,
      locale: fixture.connect.locale,
      mutation: fixture.connect.mutation,
    });

    mutationCalls = vi.mocked(invoke).mock.calls.filter(([command]) =>
      command === 'mutate_graph_document');
    expect(mutationCalls[1]).toEqual([
      'mutate_graph_document',
      {
        graphPath: fixture.graphPath,
        locale: fixture.connect.locale,
        request: {
          resource: { kind: 'graph', key: fixture.graphPath },
          baseRevision: 2,
          operationId: fixture.connect.operationId,
          payload: fixture.connect.mutation,
        },
      },
    ]);

    const projectNode = useGraphDataStore.getState().getGraphNode(
      fixture.graphPath,
      createdNodeId,
    );
    const parameter = projectNode?.parameterEditors?.find((editor) => editor.key === 'columns');
    if (!projectNode || !parameter) throw new Error('Rust projection omitted Project editor');
    expect(parameter.configuration).toMatchObject({
      kind: 'projectColumns',
      available: true,
      value: [],
    });
    act(() => root.render(
      <NodeParameterEditor
        graphPath={fixture.graphPath}
        nodeId={createdNodeId}
        locale={fixture.submit.locale}
        parameter={parameter}
        diagnostics={projectNode.diagnostics ?? []}
        formatFallback={String}
      />,
    ));

    act(() => click(container.querySelector('[aria-label="Select amount"]')));
    act(() => click(container.querySelector('[aria-label="Select region"]')));
    act(() => click(container.querySelector('[aria-label="Move region up"]')));
    await act(async () => {
      click(container.querySelector('button[type="submit"]'));
      await Promise.resolve();
    });

    mutationCalls = vi.mocked(invoke).mock.calls.filter(([command]) =>
      command === 'mutate_graph_document');
    expect(mutationCalls).toHaveLength(3);
    expect(mutationCalls[2]).toEqual([
      'mutate_graph_document',
      {
        graphPath: fixture.graphPath,
        locale: fixture.submit.locale,
        request: {
          resource: { kind: 'graph', key: fixture.graphPath },
          baseRevision: 3,
          operationId: fixture.submit.operationId,
          payload: fixture.submit.mutation,
        },
      },
    ]);
    expect(fixture.submit.selectedColumns).toEqual(['region', 'amount']);
    const updated = useGraphDataStore.getState().getGraphNode(
      fixture.graphPath,
      createdNodeId,
    );
    expect(updated?.parameterEditors?.find((editor) => editor.key === 'columns')?.configuration)
      .toMatchObject({ kind: 'projectColumns', value: fixture.submit.selectedColumns });

    const viewSource = readFileSync(
      resolve('src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.tsx'),
      'utf8',
    );
    expect(viewSource).not.toMatch(/\binvoke\s*\(/);
  });
});

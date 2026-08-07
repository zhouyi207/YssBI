import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  moduleDependencies,
  resolveSourceSpecifier,
  type ArchitectureSource,
  type ModuleDependency,
  type ModuleDependencyMode,
} from '@/tests/helpers/moduleDependencyAudit';
import type {
  EditorGraphProjectionDto,
  PortAddressDto,
} from '@/shared/types/dto/editorProjection';
import { validateEditorGraphProjection } from '@/shared/types/dto/editorProjectionParser';
import {
  portAddressKey,
  toProjectionEntities,
} from './index';

const declaredOutput: PortAddressDto = {
  kind: 'declared',
  nodeId: 'node-1',
  portKey: 'output',
};

const instanceInput: PortAddressDto = {
  kind: 'instance',
  nodeId: 'node-1',
  templateKey: 'input',
  instanceId: 'instance-1',
};

function validProjection(): EditorGraphProjectionDto {
  return {
    basis: {
      graphPath: 'functions/main',
      graphRevision: 7,
      registryFingerprint: '0101010101010101010101010101010101010101010101010101010101010101',
      resourceVersions: { 'functions/helper': '3' },
    },
    graphPath: 'functions/main',
    sourceRevision: 7,
    nodes: [
      {
        graphPath: 'functions/main',
        sourceRevision: 7,
        nodeId: 'node-1',
        nodeTypeId: 'statistics.linear-regression',
        position: { x: 120.5, y: -32 },
        display: {
          title: '线性回归',
          description: '拟合线性模型',
          userLabel: '主要模型',
          iconId: 'chart-line',
          styleId: 'analysis',
        },
        ports: [
          {
            address: declaredOutput,
            templateKey: 'output',
            display: { label: '结果', instanceLabel: null },
            direction: 'output',
            kind: 'data',
            instanceKind: 'declared',
            orphan: false,
            canRemove: false,
            connections: {
              current: 1,
              maximum: null,
              ordered: false,
              canConnect: true,
            },
            input: null,
            resolvedType: { display: 'Model', resolved: true },
            resolvedSchema: { kind: 'derived', fields: [] },
            status: 'resolved',
          },
          {
            address: instanceInput,
            templateKey: 'input',
            display: { label: '变量', instanceLabel: '变量 1' },
            direction: 'input',
            kind: 'data',
            instanceKind: 'userCreated',
            orphan: false,
            canRemove: true,
            connections: {
              current: 1,
              maximum: 1,
              ordered: false,
              canConnect: false,
            },
            input: {
              literalOverride: 42,
              protocolDefault: 0,
              effective: 'connections',
            },
            resolvedType: { display: 'Float64', resolved: true },
            resolvedSchema: { kind: 'input', fields: [] },
            status: 'resolved',
          },
        ],
        parameterEditors: [
          {
            key: 'formula',
            display: { title: '公式', description: '模型公式' },
            editor: 'text',
            multiline: true,
            value: 'y ~ x',
            configuration: null,
          },
        ],
        capabilities: {
          managed: false,
          canCopy: true,
          canDelete: true,
          canEditLabel: true,
          canEditParameters: true,
          hasDynamicPorts: true,
          supportsInlineLiterals: true,
        },
        diagnostics: [
          {
            code: 'node.warning',
            message: '节点警告',
            severity: 'warning',
            blocking: false,
            location: { kind: 'port', address: instanceInput },
            related: [{ kind: 'node', nodeId: 'node-1' }],
          },
        ],
      },
    ],
    connections: [
      {
        connectionId: 'connection-1',
        output: declaredOutput,
        input: instanceInput,
        order: 'a',
      },
    ],
    diagnostics: [
      {
        code: 'graph.info',
        message: '图诊断',
        severity: 'information',
        blocking: false,
        location: { kind: 'graph' },
        related: [{ kind: 'resource', identity: 'functions/helper' }],
      },
    ],
    hasBlockingDiagnostics: false,
  };
}

describe('editor projection architecture', () => {
  type Edge = [string, string, ModuleDependencyMode];

  const moduleSources = [
    ['dtoIndex.ts', 'src/shared/types/dto/index.ts'],
    ['editorProjection.ts', 'src/shared/types/dto/editorProjection.ts'],
    ['parameterEditorValidators.ts', 'src/shared/types/dto/parameterEditorValidators.ts'],
    ['editorProjectionGuards.ts', 'src/shared/types/dto/editorProjectionGuards.ts'],
    ['editorProjectionParser.ts', 'src/shared/types/dto/editorProjectionParser.ts'],
    ['graphProjectionService.ts', 'src/services/nodeSystem/graphProjectionService.ts'],
  ] as const;
  const dtoBarrelRuntimeTargets = [
    'src/shared/types/dto/database.ts',
    'src/shared/types/dto/project.ts',
    'src/shared/types/dto/graph.ts',
    'src/shared/types/dto/graphCommands.ts',
    'src/shared/types/dto/editorMutation.ts',
    'src/shared/types/dto/runEvent.ts',
    'src/shared/types/dto/executionDemand.ts',
    'src/shared/types/dto/trace.ts',
    'src/shared/types/dto/resultSource.ts',
    'src/shared/types/dto/converters.ts',
    'src/shared/types/dto/dataType.ts',
    'src/shared/types/dto/dataValue.ts',
    'src/shared/types/dto/variable.ts',
    'src/shared/types/dto/graphConverters.ts',
  ] as const;
  const expectedEdges = [
    ...dtoBarrelRuntimeTargets.map((target) => (
      ['dtoIndex.ts', target, 'runtime'] as Edge
    )),
    ['dtoIndex.ts', 'editorProjection.ts', 'type-only'],
    ['editorProjection.ts', 'src/shared/types/domain/graph.ts', 'type-only'],
    ['parameterEditorValidators.ts', 'editorProjection.ts', 'type-only'],
    ['editorProjectionGuards.ts', 'parameterEditorValidators.ts', 'runtime'],
    ['editorProjectionGuards.ts', 'editorProjection.ts', 'type-only'],
    ['editorProjectionGuards.ts', 'src/shared/types/domain/dataType.ts', 'type-only'],
    ['editorProjectionGuards.ts', 'src/shared/types/domain/graphResourcePath.ts', 'runtime'],
    ['editorProjectionParser.ts', 'editorProjectionGuards.ts', 'runtime'],
    ['editorProjectionParser.ts', 'parameterEditorValidators.ts', 'runtime'],
    ['editorProjectionParser.ts', 'editorProjection.ts', 'type-only'],
    ['graphProjectionService.ts', 'editorProjectionParser.ts', 'runtime'],
    ['graphProjectionService.ts', 'editorProjection.ts', 'type-only'],
    ['graphProjectionService.ts', 'external:@tauri-apps/api/core', 'runtime'],
  ] satisfies Edge[];
  const allowedRuntimeTargets = new Map<string, ReadonlySet<string>>([
    ['dtoIndex.ts', new Set(dtoBarrelRuntimeTargets)],
    ['editorProjection.ts', new Set()],
    ['parameterEditorValidators.ts', new Set()],
    ['editorProjectionGuards.ts', new Set([
      'parameterEditorValidators.ts',
      'src/shared/types/domain/graphResourcePath.ts',
    ])],
    ['editorProjectionParser.ts', new Set([
      'editorProjectionGuards.ts',
      'parameterEditorValidators.ts',
    ])],
    ['graphProjectionService.ts', new Set([
      'editorProjectionParser.ts',
      'external:@tauri-apps/api/core',
    ])],
  ]);
  const runtimeBypassFixtures = [
    ['static import', "import value from '@/features/example';"],
    ['side-effect import', "import '@/features/example';"],
    ['re-export', "export { value } from '@/services/example';"],
    ['dynamic import', "void import('@/views/example');"],
    ['CommonJS require', "require('@/features/example');"],
    ['TypeScript import equals', "import value = require('@/services/example');"],
    ['TypeScript export assignment', "export = require('@/views/example');"],
    ['alias traversal', "import value from '@/shared/../features/example';"],
    ['relative import', "import value from '../../../services/example';"],
  ] as const;
  const unresolvedRuntimeFixtures = [
    [
      'dynamic import variable',
      'dynamic-import',
      4,
      6,
      'const modulePath =\n  getModulePath();\n\nvoid import(modulePath);',
    ],
    [
      'require function call',
      'require',
      4,
      16,
      'const modulePath =\n  getModulePath();\n\nconst loaded = require(\n  getModulePath(),\n);',
    ],
    [
      'dynamic import string concatenation',
      'dynamic-import',
      4,
      6,
      "const prefix = '@/features/';\nconst name = getName();\n\nvoid import(\n  prefix + name,\n);",
    ],
    [
      'export assignment require',
      'export-assignment',
      4,
      1,
      'const modulePath =\n  getModulePath();\n\nexport = require(modulePath);',
    ],
    [
      'dynamic import attributes',
      'dynamic-import',
      4,
      6,
      "const modulePath =\n  getModulePath();\n\nvoid import(\n  modulePath,\n  { with: { type: 'json' } },\n);",
    ],
  ] as const;

  function readArchitectureSource(path: string): ArchitectureSource {
    return { path, source: readFileSync(path, 'utf8') };
  }

  function normalizedTarget(importerPath: string, specifier: string): string | null {
    return resolveSourceSpecifier(importerPath, specifier)?.replace(/\.(?:ts|tsx)$/, '') ?? null;
  }

  function unresolvedFinding(dependency: ModuleDependency): string {
    return `unresolved:${dependency.kind}@${dependency.location.line}:${dependency.location.column}`;
  }

  function actualEdges(
    fixtureModules: ReadonlyMap<string, string> = new Map(),
  ): Edge[] {
    const rootNames = new Map<string, string>(
      moduleSources.map(([name, path]) => [path, name]),
    );
    const sourceFor = (path: string): string | null => {
      const fixture = fixtureModules.get(path);
      if (fixture !== undefined) return fixture;
      return existsSync(path) && /\.tsx?$/.test(path)
        ? readFileSync(path, 'utf8')
        : null;
    };
    const displayName = (path: string): string => rootNames.get(path) ?? path;
    const edges: Edge[] = [];
    const visited = new Set<string>();
    const visit = (path: string): void => {
      if (visited.has(path)) return;
      visited.add(path);
      const source = sourceFor(path);
      if (source === null) return;

      for (const dependency of moduleDependencies(path, source)) {
        if (dependency.specifier === null) {
          edges.push([displayName(path), unresolvedFinding(dependency), dependency.mode]);
          continue;
        }
        const localTarget = resolveSourceSpecifier(
          path,
          dependency.specifier,
          resolve('src'),
          fixtureModules,
        );
        if (localTarget === null) {
          edges.push([
            displayName(path),
            `external:${dependency.specifier}`,
            dependency.mode,
          ]);
          continue;
        }
        edges.push([displayName(path), displayName(localTarget), dependency.mode]);
        if (rootNames.has(localTarget) || fixtureModules.has(localTarget)) visit(localTarget);
      }
    };

    for (const [, path] of moduleSources) visit(path);
    edges.sort((left, right) => {
      const index = (edge: Edge): number => expectedEdges.findIndex((expected) => (
        expected[0] === edge[0] && expected[1] === edge[1] && expected[2] === edge[2]
      ));
      const leftIndex = index(left);
      const rightIndex = index(right);
      if (leftIndex >= 0 || rightIndex >= 0) {
        return (leftIndex < 0 ? expectedEdges.length : leftIndex)
          - (rightIndex < 0 ? expectedEdges.length : rightIndex);
      }
      return JSON.stringify(left).localeCompare(JSON.stringify(right));
    });
    return edges;
  }

  function unexpectedRuntimeEdges(edges: readonly Edge[]): Edge[] {
    return edges.filter(([from, to, mode]) => (
      mode === 'runtime' && !(allowedRuntimeTargets.get(from)?.has(to) ?? false)
    ));
  }

  function forbiddenRuntimeTargets(
    source: ArchitectureSource,
    forbiddenTarget: RegExp,
  ): string[] {
    return moduleDependencies(source.path, source.source)
      .filter(({ mode }) => mode === 'runtime')
      .flatMap((dependency) => {
        if (dependency.specifier === null) return [unresolvedFinding(dependency)];
        const target = normalizedTarget(source.path, dependency.specifier);
        return target && forbiddenTarget.test(target) ? [target] : [];
      });
  }

  function sourceFiles(directory: string): ArchitectureSource[] {
    return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
      const path = `${directory}/${entry.name}`;
      if (entry.isDirectory()) return sourceFiles(path);
      return entry.isFile() && /\.tsx?$/.test(entry.name)
        ? [readArchitectureSource(path)]
        : [];
    });
  }

  function hasDependencyCycle(edges: readonly Edge[]): boolean {
    const adjacency = new Map<string, string[]>();
    for (const [from, to] of edges) {
      adjacency.set(from, [...(adjacency.get(from) ?? []), to]);
    }
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const visit = (node: string): boolean => {
      if (visiting.has(node)) return true;
      if (visited.has(node)) return false;
      visiting.add(node);
      if ((adjacency.get(node) ?? []).some(visit)) return true;
      visiting.delete(node);
      visited.add(node);
      return false;
    };
    return [...adjacency.keys()].some(visit);
  }

  it('resolves an unregistered local target from a fixture module map', () => {
    const fixtureModules = new Map([
      ['src/shared/types/dto/projectionRuntimeHelper.ts', 'export const helper = true;'],
    ]);
    expect(resolveSourceSpecifier(
      'src/shared/types/dto/editorProjection.ts',
      './projectionRuntimeHelper',
      resolve('src'),
      fixtureModules,
    )).toBe('src/shared/types/dto/projectionRuntimeHelper.ts');
  });

  it('retains the explicitly allowed Tauri service external', () => {
    expect(actualEdges()).toContainEqual([
      'graphProjectionService.ts',
      'external:@tauri-apps/api/core',
      'runtime',
    ]);
  });

  it('retains an unknown local runtime target as its real project path', () => {
    const fixtureModules = new Map([
      [
        'src/shared/types/dto/editorProjectionParser.ts',
        "import './projectionRuntimeHelper';",
      ],
      ['src/shared/types/dto/projectionRuntimeHelper.ts', 'export const helper = true;'],
    ]);
    const edges = actualEdges(fixtureModules);

    expect(edges).toContainEqual([
      'editorProjectionParser.ts',
      'src/shared/types/dto/projectionRuntimeHelper.ts',
      'runtime',
    ]);
    expect(unexpectedRuntimeEdges(edges)).toContainEqual([
      'editorProjectionParser.ts',
      'src/shared/types/dto/projectionRuntimeHelper.ts',
      'runtime',
    ]);
  });

  it('retains an unknown external runtime target as an unexpected edge', () => {
    const fixtureModules = new Map([
      [
        'src/shared/types/dto/parameterEditorValidators.ts',
        "import 'unknown-runtime-package';",
      ],
    ]);
    const edges = actualEdges(fixtureModules);

    expect(edges).toContainEqual([
      'parameterEditorValidators.ts',
      'external:unknown-runtime-package',
      'runtime',
    ]);
    expect(unexpectedRuntimeEdges(edges)).toContainEqual([
      'parameterEditorValidators.ts',
      'external:unknown-runtime-package',
      'runtime',
    ]);
  });

  it('reports an indirect projection back-edge as unexpected and cyclic', () => {
    const helperPath = 'src/shared/types/dto/projectionRuntimeHelper.ts';
    const fixtureModules = new Map([
      [
        'src/shared/types/dto/editorProjection.ts',
        "export { helper } from './projectionRuntimeHelper';",
      ],
      [
        helperPath,
        [
          "export { isEditorGraphProjectionDto } from './editorProjectionGuards';",
          "export { parseEditorGraphProjectionDto } from './editorProjectionParser';",
        ].join('\n'),
      ],
    ]);
    const edges = actualEdges(fixtureModules);

    expect(unexpectedRuntimeEdges(edges)).toEqual(expect.arrayContaining([
      ['editorProjection.ts', helperPath, 'runtime'],
      [helperPath, 'editorProjectionGuards.ts', 'runtime'],
      [helperPath, 'editorProjectionParser.ts', 'runtime'],
    ]));
    expect(hasDependencyCycle(edges)).toBe(true);
  });

  it.each(runtimeBypassFixtures)('rejects %s runtime dependency syntax', (_label, source) => {
    const fixture = {
      path: 'src/shared/types/dto/editorProjectionParser.ts',
      source,
    };

    expect(forbiddenRuntimeTargets(fixture, /^src\/(?:features|services|views)(?:\/|$)/))
      .toHaveLength(1);
  });

  it.each(unresolvedRuntimeFixtures)(
    'reports unresolved %s exactly and fails closed through forbidden-target glue',
    (_label, kind, line, column, source) => {
      const fixture = {
        path: 'src/shared/types/dto/editorProjectionParser.ts',
        source,
      };

      expect(moduleDependencies(fixture.path, fixture.source)).toEqual([{
        kind,
        mode: 'runtime',
        specifier: null,
        location: { line, column },
      }]);
      expect(forbiddenRuntimeTargets(
        fixture,
        /^src\/(?:features|services|views)(?:\/|$)/,
      )).toEqual([`unresolved:${kind}@${line}:${column}`]);
    },
  );

  it('rejects a null-specifier glue mutation instead of treating it as no dependency', () => {
    const fixture = {
      path: 'src/shared/types/dto/editorProjectionParser.ts',
      source: 'const target =\n  getModulePath();\n\nconst loaded = require(target);',
    };
    const [dependency] = moduleDependencies(fixture.path, fixture.source);

    expect(dependency.specifier).toBeNull();
    expect(forbiddenRuntimeTargets(
      fixture,
      /^src\/(?:features|services|views)(?:\/|$)/,
    )).toEqual(['unresolved:require@4:16']);
  });

  it.each([
    ['import type', "import type { Value } from '@/features/example';"],
    ['named type import', "import { type Value } from '@/services/example';"],
    ['type re-export', "export type { Value } from '@/views/example';"],
    ['type star re-export', "export type * from '@/features/example';"],
    ['type import equals', "import type Value = require('@/services/example');"],
  ])('allows erased %s dependencies', (_label, source) => {
    const fixture = {
      path: 'src/shared/types/dto/editorProjectionParser.ts',
      source,
    };
    const dependencies = moduleDependencies(fixture.path, fixture.source);

    expect(dependencies).toHaveLength(1);
    expect(dependencies[0].mode).toBe('type-only');
    expect(forbiddenRuntimeTargets(fixture, /^src\/(?:features|services|views)(?:\/|$)/))
      .toEqual([]);
  });

  it('enforces the complete erased and runtime projection dependency graph', () => {
    const edges = actualEdges();

    expect(edges).toEqual(expectedEdges);
    expect(edges.filter(([, , mode]) => mode === 'runtime')).toEqual([
      ...dtoBarrelRuntimeTargets.map((target) => (
        ['dtoIndex.ts', target, 'runtime'] as Edge
      )),
      ['editorProjectionGuards.ts', 'parameterEditorValidators.ts', 'runtime'],
      ['editorProjectionGuards.ts', 'src/shared/types/domain/graphResourcePath.ts', 'runtime'],
      ['editorProjectionParser.ts', 'editorProjectionGuards.ts', 'runtime'],
      ['editorProjectionParser.ts', 'parameterEditorValidators.ts', 'runtime'],
      ['graphProjectionService.ts', 'editorProjectionParser.ts', 'runtime'],
      ['graphProjectionService.ts', 'external:@tauri-apps/api/core', 'runtime'],
    ]);
    expect(unexpectedRuntimeEdges(edges)).toEqual([]);
    expect(hasDependencyCycle(edges)).toBe(false);
  });

  it('keeps every direct DTO barrel import consumer type-only', () => {
    const runtimeImportConsumers = sourceFiles('src')
      .filter(({ path }) => !/\.(?:test|spec)\.tsx?$/.test(path)
        && !/(?:^|\/)__tests__(?:\/|$)/.test(path))
      .flatMap((source) => (
        moduleDependencies(source.path, source.source).flatMap((dependency) => {
          if (dependency.mode !== 'runtime') return [];
          if (dependency.specifier === null) return [source.path];
          const target = normalizedTarget(source.path, dependency.specifier);
          return dependency.kind !== 're-export' && target === 'src/shared/types/dto'
            ? [source.path]
            : [];
        })
      ));

    expect(runtimeImportConsumers).toEqual([]);
    expect(actualEdges()).toContainEqual([
      'dtoIndex.ts', 'editorProjection.ts', 'type-only',
    ]);
  });

  it('forbids reverse runtime production dependencies', () => {
    const serviceSources = sourceFiles('src/services')
      .filter(({ path }) => !/\.(?:test|spec)\.tsx?$/.test(path));
    const sharedSources = moduleSources
      .filter(([name]) => name !== 'graphProjectionService.ts')
      .map(([, path]) => readArchitectureSource(path));
    const serviceOffenders = serviceSources
      .filter((source) => forbiddenRuntimeTargets(
        source,
        /^src\/(?:features|views)(?:\/|$)/,
      ).length > 0)
      .map(({ path }) => path);
    const sharedOffenders = sharedSources
      .filter((source) => forbiddenRuntimeTargets(
        source,
        /^src\/(?:features|services|views)(?:\/|$)/,
      ).length > 0)
      .map(({ path }) => path);

    expect(serviceOffenders).toEqual([]);
    expect(sharedOffenders).toEqual([]);
    expect(existsSync('src/features/domain/editorProjection/validateProjection.ts')).toBe(false);
  });
});


describe('portAddressKey', () => {
  it('is stable for equal addresses and distinguishes address variants', () => {
    expect(portAddressKey(declaredOutput)).toBe(portAddressKey({ ...declaredOutput }));
    expect(portAddressKey(declaredOutput)).not.toBe(portAddressKey(instanceInput));
  });

  it('does not collide when address parts contain delimiters', () => {
    const first: PortAddressDto = {
      kind: 'declared',
      nodeId: 'a:b',
      portKey: 'c',
    };
    const second: PortAddressDto = {
      kind: 'declared',
      nodeId: 'a',
      portKey: 'b:c',
    };

    expect(portAddressKey(first)).not.toBe(portAddressKey(second));
  });
});

describe('validateEditorGraphProjection', () => {
  it('returns a valid projection unchanged', () => {
    const projection = validProjection();
    expect(validateEditorGraphProjection(projection)).toBe(projection);
  });

  it.each([
    ['basis graph path', (projection: EditorGraphProjectionDto) => {
      projection.basis.graphPath = 'functions/other';
    }],
    ['node graph path', (projection: EditorGraphProjectionDto) => {
      projection.nodes[0].graphPath = 'functions/other';
    }],
    ['basis revision', (projection: EditorGraphProjectionDto) => {
      projection.basis.graphRevision = 8;
    }],
    ['node revision', (projection: EditorGraphProjectionDto) => {
      projection.nodes[0].sourceRevision = 8;
    }],
  ])('rejects mismatched %s', (_, mutate) => {
    const projection = validProjection();
    mutate(projection);
    expect(() => validateEditorGraphProjection(projection)).toThrow(/does not match/);
  });

  it('rejects duplicate node, port, and connection identities', () => {
    const duplicateNode = validProjection();
    duplicateNode.nodes.push(structuredClone(duplicateNode.nodes[0]));
    expect(() => validateEditorGraphProjection(duplicateNode)).toThrow(/duplicate node/);

    const duplicatePort = validProjection();
    duplicatePort.nodes[0].ports.push(structuredClone(duplicatePort.nodes[0].ports[0]));
    expect(() => validateEditorGraphProjection(duplicatePort)).toThrow(/duplicate port/);

    const duplicateConnection = validProjection();
    duplicateConnection.connections.push(structuredClone(duplicateConnection.connections[0]));
    expect(() => validateEditorGraphProjection(duplicateConnection)).toThrow(/duplicate connection/);
  });

  it('strictly validates Rust-issued schema-aware editor wire data', () => {
    const projection = validProjection();
    projection.nodes[0].parameterEditors[0].configuration = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'amount',
        dataType: 'float64',
        operators: ['equal', 'greaterThan', 'isNull'],
        literalTypes: ['integer', 'decimal'],
      }],
      value: {
        column: 'amount',
        operator: 'greaterThan',
        value: { type: 'decimal', value: '9007199254740993.5' },
      },
    };
    expect(validateEditorGraphProjection(projection)).toBe(projection);

    const extra = structuredClone(projection);
    Object.assign(extra.nodes[0].parameterEditors[0].configuration!, { compatibility: true });
    expect(() => validateEditorGraphProjection(extra)).toThrow(/parameter editor/);

    const lossy = structuredClone(projection);
    const configuration = lossy.nodes[0].parameterEditors[0].configuration;
    if (configuration?.kind !== 'filterPredicate' || !configuration.value?.value) {
      throw new Error('test fixture mismatch');
    }
    configuration.value.value.value = 9007199254740994 as never;
    expect(() => validateEditorGraphProjection(lossy)).toThrow(/parameter editor/);
  });

  it('rejects a port address owned by a different node', () => {
    const projection = validProjection();
    projection.nodes[0].ports[0].address = {
      ...declaredOutput,
      nodeId: 'node-2',
    };

    expect(() => validateEditorGraphProjection(projection)).toThrow(/owned by node 'node-2'/);
  });

  it('rejects connections that reference a missing port', () => {
    const missingEndpointProjection = validProjection();
    missingEndpointProjection.connections[0].input = {
      kind: 'declared',
      nodeId: 'node-1',
      portKey: 'missing',
    };

    expect(() => validateEditorGraphProjection(missingEndpointProjection)).toThrow(
      "projection connection 'connection-1' references a missing port",
    );
  });

  it.each([
    ['output', 0, 'input'],
    ['input', 1, 'output'],
  ] as const)('rejects a connection whose %s endpoint has the wrong direction', (_, portIndex, direction) => {
    const wrongDirectionProjection = validProjection();
    wrongDirectionProjection.nodes[0].ports[portIndex].direction = direction;

    expect(() => validateEditorGraphProjection(wrongDirectionProjection)).toThrow(
      /connection 'connection-1'.*direction/,
    );
  });
});

describe('toProjectionEntities', () => {
  it('converts a valid projection without a registry and preserves projected data', () => {
    const projection = validProjection();
    const entities = toProjectionEntities(projection);
    const outputKey = portAddressKey(declaredOutput);
    const inputKey = portAddressKey(instanceInput);

    expect(entities.basis).toEqual(projection.basis);
    expect(entities.graphPath).toBe('functions/main');
    expect(entities.sourceRevision).toBe(7);
    expect(entities.nodes['node-1']).toMatchObject({
      nodeTypeId: 'statistics.linear-regression',
      position: { x: 120.5, y: -32 },
      display: { title: '线性回归', userLabel: '主要模型' },
      parameterEditors: projection.nodes[0].parameterEditors,
      diagnostics: projection.nodes[0].diagnostics,
    });
    expect(entities.ports[outputKey].address).toEqual(declaredOutput);
    expect(entities.ports[inputKey]).toMatchObject({
      address: instanceInput,
      input: {
        literalOverride: 42,
        protocolDefault: 0,
        effective: 'connections',
      },
    });
    expect(entities.connections['connection-1']).toEqual(projection.connections[0]);
    expect(entities.portKeysByNodeId['node-1']).toEqual([outputKey, inputKey]);
    expect(entities.connectionIdsByPortKey[outputKey]).toEqual(['connection-1']);
    expect(entities.connectionIdsByPortKey[inputKey]).toEqual(['connection-1']);
    expect(entities.diagnostics).toEqual(projection.diagnostics);
    expect(entities.hasBlockingDiagnostics).toBe(false);
  });
});

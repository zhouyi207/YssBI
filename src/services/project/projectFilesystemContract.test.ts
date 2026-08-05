import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  moduleDependencies,
  resolveSourceSpecifier,
  type ArchitectureSource,
} from '@/tests/helpers/moduleDependencyAudit';

const sourceRoot = resolve('src');



const legacyIdentityFacadePaths = [
  'src/services/project/projectIdentity.ts',
  'src/features/application/projectIdentity.ts',
] as const;
const legacyIdentityTargets = new Set(
  legacyIdentityFacadePaths.map((path) => path.replace(/\.ts$/, '')),
);
const projectPublicationTarget =
  'src/features/application/editorMutation/projectPublicationCoordinator';



function hasUnresolvedRuntimeDependency({ path, source }: ArchitectureSource): boolean {
  return moduleDependencies(path, source).some((dependency) => (
    dependency.mode === 'runtime' && dependency.specifier === null
  ));
}

function resolvedSourceTargets({ path, source }: ArchitectureSource): string[] {
  return moduleDependencies(path, source).flatMap(({ specifier }) => {
    if (specifier === null) return [];
    const target = resolveSourceSpecifier(path, specifier);
    return target === null ? [] : [target.replace(/\.(?:ts|tsx)$/, '')];
  });
}

function readArchitectureSource(path: string): ArchitectureSource {
  return { path, source: readFileSync(resolve(path), 'utf8') };
}

function serviceBoundaryViolations(sources: readonly ArchitectureSource[]): string[] {
  return sources
    .filter((source) => hasUnresolvedRuntimeDependency(source)
      || resolvedSourceTargets(source).some((target) => (
        /^src\/(?:features|views)(?:\/|$)/.test(target)
      )))
    .map(({ path }) => path);
}

function legacyIdentityViolations(
  sources: readonly ArchitectureSource[],
  restoredFacadePaths: readonly string[] = [],
): string[] {
  const sourceViolations = sources
    .filter((source) => hasUnresolvedRuntimeDependency(source)
      || resolvedSourceTargets(source).some((target) => legacyIdentityTargets.has(target)))
    .map(({ path }) => path);
  return [
    ...restoredFacadePaths.filter((path) => legacyIdentityFacadePaths.includes(
      path as typeof legacyIdentityFacadePaths[number],
    )),
    ...sourceViolations,
  ];
}

function graphProjectionLifecycleViolations(
  sources: readonly ArchitectureSource[],
): string[] {
  return sources
    .filter((source) => hasUnresolvedRuntimeDependency(source)
      || resolvedSourceTargets(source).some((target) => (
        target === projectPublicationTarget || legacyIdentityTargets.has(target)
      )))
    .map(({ path }) => path);
}

function isProductionSourcePath(path: string): boolean {
  const normalizedPath = path.replace(/\\/g, '/');
  return ['.ts', '.tsx'].includes(extname(normalizedPath))
    && !/\.(?:test|spec)\.[^/]+$/.test(normalizedPath)
    && !/(?:^|\/)__tests__(?:\/|$)/.test(normalizedPath);
}

function productionSources(directory = sourceRoot): Array<{ path: string; source: string }> {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionSources(path);
    if (!isProductionSourcePath(path)) return [];
    return [{ path: relative(resolve('.'), path).replace(/\\/g, '/'), source: readFileSync(path, 'utf8') }];
  });
}

const activeProjectCommandIdentityFields = {
  get_project_databases_variables: 'projectInstanceId',
  get_project_path: 'projectInstanceId',
  get_project_index: 'projectInstanceId',
  get_project_resource_path: 'projectInstanceId',
  load_project_graph: 'projectInstanceId',
  hydrate_editor_graph: 'projectInstanceId',
  unload_project_graph: 'projectInstanceId',
  save_project_graph: 'projectInstanceId',
  flush_project: 'projectInstanceId',
  save_project_as: 'projectInstanceId',
  delete_registered_project_files: 'expectedActiveProjectInstanceId',
  create_event: 'projectInstanceId',
  create_function: 'projectInstanceId',
  duplicate_graph: 'projectInstanceId',
  remove_graph: 'projectInstanceId',
  rename_graph_resource: 'projectInstanceId',
  create_variable: 'projectInstanceId',
  get_variable: 'projectInstanceId',
  update_variable: 'projectInstanceId',
  delete_variable: 'projectInstanceId',
  create_worksheet: 'projectInstanceId',
  load_worksheet: 'projectInstanceId',
  save_worksheet: 'projectInstanceId',
  delete_worksheet: 'projectInstanceId',
} as const;

const activeProjectCommands = Object.keys(activeProjectCommandIdentityFields) as Array<
  keyof typeof activeProjectCommandIdentityFields
>;

const workflowFiles = [
  'src/features/core/dataStore/projectIOStore.ts',
  'src/features/application/editorProjection/graphProjectionCoordinator.ts',
  'src/features/application/editor/graphDocumentUnload.ts',
  'src/features/application/editor/closeGraphTab.ts',
  'src/features/application/editor/saveAllDirtyGraphs.ts',

  'src/features/application/editor/useProjectOperations.ts',
  'src/features/application/editor/useWorksheetManagement.ts',
  'src/features/application/dataManagement/variableActions.ts',
  'src/features/application/resource/resourceActions.ts',
  'src/features/application/project/useProjectPicker.ts',
] as const;

function invokePayload(source: string, command: string): string | null {
  const commandOffsets = [`'${command}'`, `"${command}"`]
    .map((literal) => source.indexOf(literal))
    .filter((offset) => offset >= 0);
  if (commandOffsets.length === 0) return null;
  const commandOffset = Math.min(...commandOffsets);
  const invokeOffset = source.lastIndexOf('invoke', commandOffset);
  if (invokeOffset < 0 || commandOffset - invokeOffset > 500) return null;
  return source.slice(invokeOffset, invokeOffset + 700);
}

describe('projectFilesystemContract', () => {
  const forbiddenServiceImportFixtures: Array<[string, ArchitectureSource]> = [
    ['static import with bindings', {
      path: 'src/services/project/staticFixture.ts',
      source: "import value from '@/features/core/example';",
    }],
    ['side-effect import', {
      path: 'src/services/project/sideEffectFixture.ts',
      source: "import '@/features/core/example';",
    }],
    ['dynamic import', {
      path: 'src/services/project/dynamicFixture.ts',
      source: "const value = await import('@/views/example');",
    }],
    ['dynamic import with attributes', {
      path: 'src/services/project/dynamicAttributesFixture.ts',
      source: "const value = await import('@/views/example.json', { with: { type: 'json' } });",
    }],
    ['CommonJS require', {
      path: 'src/services/project/requireFixture.ts',
      source: "const value = require('@/features/core/example');",
    }],
    ['TypeScript import equals require', {
      path: 'src/services/project/importEqualsFixture.ts',
      source: "import value = require('@/features/application/example');",
    }],
    ['relative import', {
      path: 'src/services/project/relativeFixture.ts',
      source: "import value from '../../features/core/example';",
    }],
    ['re-export', {
      path: 'src/services/project/reExportFixture.ts',
      source: "export { value } from '@/views/example';",
    }],
    ['alias traversal into features', {
      path: 'src/services/project/aliasFeatureTraversalFixture.ts',
      source: "import value from '@/shared/../features/core/example';",
    }],
    ['alias traversal into views', {
      path: 'src/services/project/aliasViewTraversalFixture.ts',
      source: "export { value } from '@/services/../views/example';",
    }],
  ];

  it.each(forbiddenServiceImportFixtures)('rejects %s service boundary violation', (_, fixture) => {
    expect(serviceBoundaryViolations([fixture])).toEqual([fixture.path]);
  });

  it('allows service and shared imports in the scoped service audit', () => {
    const fixtures: ArchitectureSource[] = [
      {
        path: 'src/services/project/allowedServiceFixture.ts',
        source: "import { GraphService } from '@/services/graph/graphService';",
      },
      {
        path: 'src/services/project/allowedSharedFixture.ts',
        source: "export type { ProjectIndexDto } from '../../shared/types/dto/project';",
      },
    ];

    expect(serviceBoundaryViolations(fixtures)).toEqual([]);
  });

  it.each([
    ['dynamic import variable', "const path = getModulePath(); void import(path);"],
    ['require function call', 'require(getModulePath());'],
    ['dynamic import string concatenation', "void import('@/features/' + name);"],
    ['export assignment require', 'export = require(modulePath);'],
    [
      'dynamic import attributes',
      "void import(modulePath, { with: { type: 'json' } });",
    ],
  ])('rejects unresolved %s runtime dependency', (_label, source) => {
    const fixture = {
      path: 'src/services/project/unresolvedFixture.ts',
      source,
    };

    expect(serviceBoundaryViolations([fixture])).toEqual([fixture.path]);
  });

  it('centralizes production source filtering for tests, specs, and test directories', () => {
    expect([
      'src/services/example.ts',
      'src/services/example.tsx',
      'src/services/example.test.ts',
      'src/services/example.spec.tsx',
      'src/services/__tests__/helper.ts',
      'src/services/example.js',
    ].map(isProductionSourcePath)).toEqual([
      true,
      true,
      false,
      false,
      false,
      false,
    ]);
  });

  it('keeps production services independent from features and views', () => {
    const serviceSources = productionSources(resolve('src/services'));

    expect(serviceSources).toHaveLength(34);
    expect(serviceBoundaryViolations(serviceSources)).toEqual([]);
  });

  it('rejects restored legacy identity facades and import or re-export shims', () => {
    const fixtures: ArchitectureSource[] = [
      {
        path: 'src/features/application/serviceShim.ts',
        source: "import { captureProjectIdentity } from '@/services/project/projectIdentity';",
      },
      {
        path: 'src/features/application/applicationShim.ts',
        source: "export * from '@/features/application/projectIdentity';",
      },
      {
        path: 'src/features/application/relativeShim.ts',
        source: "export { captureProjectIdentity } from './projectIdentity';",
      },
    ];
    const restoredFacadePaths = [...legacyIdentityFacadePaths];

    expect(legacyIdentityViolations(fixtures, restoredFacadePaths)).toEqual([
      ...restoredFacadePaths,
      ...fixtures.map(({ path }) => path),
    ]);
  });

  it('keeps both historical identity facades absent without import or re-export shims', () => {
    const restoredFacadePaths = legacyIdentityFacadePaths
      .filter((path) => existsSync(resolve(path)));

    expect(restoredFacadePaths).toEqual([]);
    expect(legacyIdentityViolations(productionSources(), restoredFacadePaths)).toEqual([]);
  });

  it.each([
    ['publication reverse import', {
      path: 'src/features/application/editorProjection/importReverseEdgeFixture.ts',
      source: "import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';",
    }],
    ['publication reverse re-export', {
      path: 'src/features/application/editorProjection/reExportReverseEdgeFixture.ts',
      source: "export * from '../editorMutation/projectPublicationCoordinator';",
    }],
    ['application identity facade re-export', {
      path: 'src/features/application/editorProjection/identityReExportFixture.ts',
      source: "export * from '@/shared/../features/application/projectIdentity';",
    }],
  ] satisfies Array<[string, ArchitectureSource]>)(
    'rejects graph projection %s lifecycle edge',
    (_, fixture) => {
      expect(graphProjectionLifecycleViolations([fixture])).toEqual([fixture.path]);
    },
  );

  it('keeps lifecycle authority dependency-safe and consumed by identity coordinators', () => {
    const authority = readArchitectureSource(
      'src/features/core/projectLifecycle/projectLifecycleAuthority.ts',
    );
    const publication = readArchitectureSource(
      'src/features/application/editorMutation/projectPublicationCoordinator.ts',
    );
    const graphProjection = readArchitectureSource(
      'src/features/application/editorProjection/graphProjectionCoordinator.ts',
    );
    const authorityForbiddenTargets = resolvedSourceTargets(authority).filter((target) =>
      /^src\/(?:features\/application|services|views)(?:\/|$)/.test(target));
    const authorityTarget = 'src/features/core/projectLifecycle/projectLifecycleAuthority';
    const graphProjectionTarget =
      'src/features/application/editorProjection/graphProjectionCoordinator';

    expect(authorityForbiddenTargets).toEqual([]);
    expect(resolvedSourceTargets(publication)).toContain(authorityTarget);
    expect(resolvedSourceTargets(publication)).toContain(graphProjectionTarget);
    expect(resolvedSourceTargets(graphProjection)).toContain(authorityTarget);
    expect(graphProjectionLifecycleViolations([graphProjection])).toEqual([]);
  });

  it('sends projectInstanceId for every active-project read and write', () => {
    const serviceSources = productionSources(resolve('src/services'));
    const offenders = activeProjectCommands.flatMap((command) => {
      const invokes = serviceSources.flatMap(({ path, source }) => {
        const payload = invokePayload(source, command);
        return payload === null ? [] : [{ path, payload }];
      });
      if (invokes.length === 0) return [`missing service invoke: ${command}`];
      const identityField = activeProjectCommandIdentityFields[command];
      return invokes.flatMap(({ path, payload }) =>
        new RegExp(`${identityField}\\s*[,}]`).test(payload)
          ? []
          : [`${path}: ${command} missing ${identityField}`]);
    });

    expect(offenders).toEqual([]);
  });

  it('contains no direct invoke outside services for project filesystem commands', () => {
    const commandPattern = new RegExp(activeProjectCommands.join('|')); 
    const offenders = productionSources()
      .filter(({ path }) => !path.startsWith('src/services/'))
      .filter(({ source }) => /@tauri-apps\/api\/core/.test(source) && /\binvoke\s*(?:<|\()/.test(source))
      .filter(({ source }) => commandPattern.test(source))
      .map(({ path }) => path);

    expect(offenders).toEqual([]);
  });

  it('rejects stale direct results before any frontend side effect', () => {
    const offenders = workflowFiles.filter((path) => {
      const source = readFileSync(resolve(path), 'utf8');
      if (!/await\s+(?:ProjectService|GraphService|GraphProjectionService|VariableService|WorksheetService)\./.test(source)) {
        return false;
      }
      const usesIdentityFacade = source.includes('captureProjectIdentity')
        && (source.includes('isCurrentProjectIdentity')
          || source.includes('assertCurrentProjectIdentity'));
      const usesCommandContextFacade = (source.includes('captureProjectCommandContext')
        || source.includes('captureGraphSaveCommandContext')
        || source.includes('captureRevisionedProjectCommandSnapshot'))
        && (source.includes('.isCurrent()')
          || source.includes('.assertCurrent()')
          || source.includes('isGraphSaveCommandRevisionCurrent'));
      const usesLifecycleReceiptOwner = source.includes('registerPendingProjectLifecycleOperation')
        && source.includes('.isCurrent()');
      return !usesIdentityFacade && !usesCommandContextFacade && !usesLifecycleReceiptOwner;
    });

    expect(offenders).toEqual([]);
  });

  it('rejects stale events before correlation or store access', () => {
    const eventFiles = [
      'src/features/core/sync/handlers/ResourceEventHandler.ts',
      'src/features/core/sync/handlers/ProjectMutationEventHandler.ts',
    ];
    const forbiddenBeforeIdentity = [
      'getPendingMutation(',
      'useGraphDataStore.getState(',
      'useResourceStore.getState(',
      'notifyIndexInvalidated(',
      'projectPublicationCoordinator.submit(',
    ];
    const offenders = eventFiles.flatMap((path) => {
      const source = readFileSync(resolve(path), 'utf8');
      const handlers = source.split(/\nexport class /).slice(1);
      return handlers.flatMap((handler) => {
        const guard = handler.indexOf('isCurrentProjectEvent(');
        const effect = forbiddenBeforeIdentity
          .map((token) => handler.indexOf(token))
          .filter((offset) => offset >= 0)
          .sort((a, b) => a - b)[0];
        return guard < 0 || (effect !== undefined && effect < guard)
          ? [`${path}: ${handler.slice(0, handler.indexOf(' '))}`]
          : [];
      });
    });

    expect(offenders).toEqual([]);
  });

  it('binds project activation direct and event wires to the backend identity', () => {
    const service = readFileSync(resolve('src/services/project/projectService.ts'), 'utf8');
    const command = readFileSync(
      resolve('src-tauri/src/commands/command_project/lifecycle.rs'),
      'utf8',
    );
    const event = readFileSync(
      resolve('src-tauri/src/event/event_project.rs'),
      'utf8',
    ).replace(/\r\n/g, '\n');

    expect(service).toContain('Promise<ProjectActivationResult>');
    expect(command).toContain('Result<ProjectActivationResultDto, FrontendError>');
    expect(command).toContain('project_instance_id: session.instance_id.to_string()');
    expect(command).toContain('activation_revision: state.activation_revision()');
    expect(event).toContain('ProjectLoaded {\n        result: ProjectActivationResultDto');
  });

  it('contains no optional projectInstanceId in active-project service contracts', () => {
    const offenders = productionSources(resolve('src/services')).flatMap(({ path, source }) => {
      const matches = source.match(/projectInstanceId\s*\?\s*:\s*string|projectInstanceId\s*:\s*string\s*\|\s*(?:null|undefined)|projectInstanceId\s*=\s*[^,;)]+/g) ?? [];
      return matches.map((match) => `${path}: ${match}`);
    });

    const publicationCoordinator = readFileSync(
      resolve('src/features/application/editorMutation/projectPublicationCoordinator.ts'),
      'utf8',
    );

    expect(offenders).toEqual([]);
    const publicationState = publicationCoordinator.match(
      /interface ProjectPublicationState \{[\s\S]*?\n\}/,
    )?.[0] ?? '';
    expect(publicationState).not.toMatch(/^\s*(?:projectInstanceId|epoch|activationRevision):/m);
  });
});

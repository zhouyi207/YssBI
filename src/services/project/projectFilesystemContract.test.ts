import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import ts from 'typescript';
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

const lifecycleOwnedNodeCommandIdentityFields = {
  mutate_graph_document: 'projectInstanceId',
  update_function_signature: 'projectInstanceId',
  hydrate_editor_graph: 'projectInstanceId',
  get_project_history_status: 'projectInstanceId',
  undo_graph_document: 'projectInstanceId',
  redo_graph_document: 'projectInstanceId',
  execute_graph_document: 'projectInstanceId',
} as const;

const activeProjectCommandIdentityFields = {
  get_localized_node_catalog: 'projectInstanceId',
  get_project_databases_variables: 'projectInstanceId',
  get_project_path: 'projectInstanceId',
  get_project_index: 'projectInstanceId',
  get_project_resource_path: 'projectInstanceId',
  load_project_graph: 'projectInstanceId',
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
  ...lifecycleOwnedNodeCommandIdentityFields,
} as const;

const activeProjectCommands = Object.keys(activeProjectCommandIdentityFields) as Array<
  keyof typeof activeProjectCommandIdentityFields
>;

const bootstrapCommandExemptions = [
  'get_current_project_activation',
  'default_project_parent_directory',
  'validate_new_project_path',
  'list_registered_projects',
  'scan_projects_in_directory',
  'cancel_project_picker_task',
  'cleanup_invalid_registered_projects',
  'register_project',
  'remove_registered_project',
  'toggle_registered_project_favorite',
  'get_project_registry_path',
  'create_project',
  'new_project',
  'load_project',
] as const;

const globalCommandExemptions = [
  'get_window_states',
  'get_window_state',
  'save_window_state',
  'list_sqlite_tables',
  'list_sql_tables',
  'list_excel_sheets',
  'hypothesis_test',
  'parse_at_values',
  'compute_acf_pacf',
  'compute_serial_tests',
  'compute_panel_did_fake_group_ri',
  'parse_bayes_expression',
  'validate_bayes_model',
  'get_julia_runtime_status',
  'get_julia_worker_status',
  'install_julia_runtime',
  'frontend_log',
  'get_logs',
  'get_log_file_path',
  'get_log_count',
] as const;

const capabilityCommandExemptions = [
  'cancel_graph_run',
  'list_graph_traces',
  'get_run_trace',
  'get_result_source_descriptor',
  'get_result_source_value',
  'get_result_source_page',
  'release_result_source',
  'release_run_result_sources',
  'load_database',
  'delete_database',
  'rename_database',
  'get_database_meta',
  'get_database_rows',
  'get_column_stats',
  'get_column_distribution',
  'get_dataset_overview',
  'edit_cell',
  'add_row',
  'delete_rows',
  'add_column',
  'delete_column',
  'cast_column',
  'rename_column',
  'undo_edit',
  'redo_edit',
  'save_database_changes',
  'export_database',
  'get_edit_state',
  'get_plot_column_pair',
  'submit_bayes_inference',
  'get_bayes_inference_status',
  'cancel_bayes_inference',
  'read_bayes_inference_result',
  'clear_bayes_inference_task',
  'export_bayes_artifact_csv',
  'read_bayes_posterior_samples',
  'read_bayes_trace_plot_data',
  'read_bayes_density_plot_data',
  'read_bayes_autocorrelation_data',
  'read_bayes_posterior_predictive',
] as const;

const identityExemptCommands = [
  ...bootstrapCommandExemptions,
  ...globalCommandExemptions,
  ...capabilityCommandExemptions,
] as const;

function registeredTauriCommands(source: string): string[] {
  const handler = source.match(/tauri::generate_handler!\[([\s\S]*?)\]\)/)?.[1] ?? '';
  return handler
    .replace(/\/\/.*$/gm, '')
    .split(',')
    .map((command) => command.trim())
    .filter(Boolean);
}

interface ServiceInvoke {
  command: string;
  path: string;
  payloadFields: string[] | null;
}

function objectLiteralFields(node: ts.Expression | undefined): string[] | null {
  if (!node || !ts.isObjectLiteralExpression(node)) return null;
  return node.properties.flatMap((property) => {
    if (ts.isShorthandPropertyAssignment(property)) return [property.name.text];
    if (!ts.isPropertyAssignment(property)) return [];
    return ts.isIdentifier(property.name) || ts.isStringLiteralLike(property.name)
      ? [property.name.text]
      : [];
  });
}

function serviceInvokes(sources: readonly ArchitectureSource[]): ServiceInvoke[] {
  return sources.flatMap(({ path, source }) => {
    const sourceFile = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
    const invokes: ServiceInvoke[] = [];
    const visit = (node: ts.Node): void => {
      if (ts.isCallExpression(node)
        && ts.isIdentifier(node.expression)
        && node.expression.text === 'invoke'
        && node.arguments.length > 0
        && ts.isStringLiteralLike(node.arguments[0])) {
        invokes.push({
          command: node.arguments[0].text,
          path,
          payloadFields: objectLiteralFields(node.arguments[1]),
        });
      }
      ts.forEachChild(node, visit);
    };
    visit(sourceFile);
    return invokes;
  });
}

function activeProjectInvokeIdentityViolations(
  invokes: readonly ServiceInvoke[],
  identityFields: Readonly<Record<string, string>> = activeProjectCommandIdentityFields,
): string[] {
  return Object.entries(identityFields).flatMap(([command, identityField]) => {
    const commandInvokes = invokes.filter((invoke) => invoke.command === command);
    if (commandInvokes.length === 0) return [`missing service invoke: ${command}`];
    return commandInvokes.flatMap(({ path, payloadFields }) =>
      payloadFields?.includes(identityField)
        ? []
        : [`${path}: ${command} missing ${identityField}`]);
  });
}

function rustCommandSources(directory = resolve('src-tauri/src/commands')): ArchitectureSource[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return rustCommandSources(path);
    if (extname(path) !== '.rs') return [];
    return [{ path: relative(resolve('.'), path).replace(/\\/g, '/'), source: readFileSync(path, 'utf8') }];
  });
}

function rustCommandSignature(
  sources: readonly ArchitectureSource[],
  command: string,
): { path: string; signature: string } | null {
  const declaration = new RegExp(`^pub\\s+(?:async\\s+)?fn\\s+${command}\\s*\\(`, 'm');
  for (const { path, source } of sources) {
    const match = declaration.exec(source);
    if (!match) continue;
    const parametersStart = source.indexOf('(', match.index);
    let depth = 0;
    for (let offset = parametersStart; offset < source.length; offset += 1) {
      if (source[offset] === '(') depth += 1;
      if (source[offset] === ')') depth -= 1;
      if (depth === 0) return { path, signature: source.slice(parametersStart + 1, offset) };
    }
  }
  return null;
}

function toSnakeCase(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function unwrapParentheses(expression: ts.Expression): ts.Expression {
  let current = expression;
  while (ts.isParenthesizedExpression(current)) current = current.expression;
  return current;
}

function negatedExpression(expression: ts.Expression): ts.Expression | null {
  const unwrapped = unwrapParentheses(expression);
  return ts.isPrefixUnaryExpression(unwrapped)
    && unwrapped.operator === ts.SyntaxKind.ExclamationToken
    ? unwrapParentheses(unwrapped.operand)
    : null;
}

function returnsImmediately(statement: ts.Statement): boolean {
  if (ts.isReturnStatement(statement)) return true;
  return ts.isBlock(statement)
    && statement.statements.length === 1
    && ts.isReturnStatement(statement.statements[0]);
}

function capturedIdentityNames(statement: ts.Statement): string[] {
  if (!ts.isVariableStatement(statement)) return [];
  return statement.declarationList.declarations.flatMap((declaration) => {
    if (!ts.isIdentifier(declaration.name) || !declaration.initializer) return [];
    const initializer = unwrapParentheses(declaration.initializer);
    return ts.isCallExpression(initializer)
      && ts.isIdentifier(initializer.expression)
      && initializer.expression.text === 'captureCurrentProjectEventIdentity'
      ? [declaration.name.text]
      : [];
  });
}

function isIdentityGuard(
  statement: ts.Statement,
  capturedIdentities: ReadonlySet<string>,
): boolean {
  if (!ts.isIfStatement(statement)
    || statement.elseStatement
    || !returnsImmediately(statement.thenStatement)) return false;
  const guardedExpression = negatedExpression(statement.expression);
  if (guardedExpression === null) return false;
  if (ts.isIdentifier(guardedExpression)) {
    return capturedIdentities.has(guardedExpression.text);
  }
  return ts.isCallExpression(guardedExpression)
    && ts.isIdentifier(guardedExpression.expression)
    && guardedExpression.expression.text === 'isCurrentProjectEvent';
}

function isForbiddenHandlerEffect(node: ts.CallExpression): boolean {
  if (ts.isIdentifier(node.expression)) {
    return node.expression.text === 'getPendingMutation'
      || node.expression.text === 'notifyIndexInvalidated';
  }
  if (!ts.isPropertyAccessExpression(node.expression)) return false;
  const owner = node.expression.expression;
  const member = node.expression.name.text;
  return ts.isIdentifier(owner)
    && ((owner.text === 'useGraphDataStore' && member === 'getState')
      || (owner.text === 'useResourceStore' && member === 'getState')
      || (owner.text === 'projectPublicationCoordinator' && member === 'submit'));
}

function containsForbiddenHandlerEffect(statement: ts.Statement): boolean {
  let forbidden = false;
  const visit = (node: ts.Node): void => {
    if (forbidden || (node !== statement && ts.isFunctionLike(node))) return;
    if (ts.isCallExpression(node) && isForbiddenHandlerEffect(node)) {
      forbidden = true;
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(statement);
  return forbidden;
}

function handleHasIdentityGuardBeforeEffects(body: ts.Block): boolean {
  const capturedIdentities = new Set<string>();
  for (const statement of body.statements) {
    if (isIdentityGuard(statement, capturedIdentities)) return true;
    if (containsForbiddenHandlerEffect(statement)) return false;
    capturedIdentityNames(statement).forEach((name) => capturedIdentities.add(name));
  }
  return false;
}

function eventHandlerIdentityGuardViolations(path: string, source: string): string[] {
  const sourceFile = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
  const violations: string[] = [];
  sourceFile.forEachChild((node) => {
    if (!ts.isClassDeclaration(node) || !node.name) return;
    const handle = node.members.find((member): member is ts.MethodDeclaration =>
      ts.isMethodDeclaration(member)
      && ts.isIdentifier(member.name)
      && member.name.text === 'handle');
    if (!handle?.body || !handleHasIdentityGuardBeforeEffects(handle.body)) {
      violations.push(`${path}: ${node.name.text}`);
    }
  });
  return violations;
}

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

  it('classifies every registered Tauri command without duplicate or stale exemptions', () => {
    const registered = registeredTauriCommands(
      readFileSync(resolve('src-tauri/src/lib.rs'), 'utf8'),
    );
    const classified = [...activeProjectCommands, ...identityExemptCommands];
    const duplicates = classified.filter((command, index) => classified.indexOf(command) !== index);
    const unclassified = registered.filter((command) => !classified.includes(
      command as typeof classified[number],
    ));
    const staleClassifications = classified.filter((command) => !registered.includes(command));

    expect({ duplicates, unclassified, staleClassifications }).toEqual({
      duplicates: [],
      unclassified: [],
      staleClassifications: [],
    });
  });

  it('extracts invoke payload keys semantically without matching comments or values', () => {
    const invokes = serviceInvokes([{
      path: 'src/services/project/fixture.ts',
      source: `
        // invoke('execute_graph_document', { projectInstanceId });
        invoke('execute_graph_document', { other: projectInstanceId });
      `,
    }]);

    expect(invokes).toEqual([{
      command: 'execute_graph_document',
      path: 'src/services/project/fixture.ts',
      payloadFields: ['other'],
    }]);
  });

  it('classifies localized catalog reads as active-project identity-required', () => {
    expect(activeProjectCommandIdentityFields).toMatchObject({
      get_localized_node_catalog: 'projectInstanceId',
    });
    expect(bootstrapCommandExemptions).not.toContain('get_localized_node_catalog');
  });

  it.each([
    ['removed', "invoke('get_localized_node_catalog', { locale: 'en-US' });"],
    ['renamed', "invoke('get_localized_node_catalog', { projectId: projectInstanceId, locale: 'en-US' });"],
  ])('detects %s localized catalog payload identity', (_, source) => {
    const path = 'src/services/nodeSystem/catalogMutationFixture.ts';
    const invokes = serviceInvokes([{ path, source }]);

    expect(activeProjectInvokeIdentityViolations(invokes, {
      get_localized_node_catalog: 'projectInstanceId',
    })).toEqual([
      `${path}: get_localized_node_catalog missing projectInstanceId`,
    ]);
  });

  it('sends the required identity field in every active-project service invoke', () => {
    const invokes = serviceInvokes(productionSources(resolve('src/services')));

    expect(activeProjectInvokeIdentityViolations(invokes)).toEqual([]);
  });

  it('requires direct identity parameters in the seven lifecycle-owned Rust commands', () => {
    const sources = rustCommandSources();
    const commands = Object.keys(lifecycleOwnedNodeCommandIdentityFields) as Array<
      keyof typeof lifecycleOwnedNodeCommandIdentityFields
    >;
    const offenders = commands.flatMap((command) => {
      const declaration = rustCommandSignature(sources, command);
      if (declaration === null) return [`missing Rust command signature: ${command}`];
      const identityParameter = toSnakeCase(lifecycleOwnedNodeCommandIdentityFields[command]);
      return new RegExp(`\\b${identityParameter}\\s*:\\s*ProjectInstanceId\\b`)
        .test(declaration.signature)
        ? []
        : [`${declaration.path}: ${command} missing required ${identityParameter}`];
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

  it.each([
    ['direct guard formatting', `
      export class DirectGuardHandler {
        handle(payload: Payload): void {
          if (
            !isCurrentProjectEvent(
              payload.projectInstanceId,
            )
          ) {
            return;
          }
          notifyIndexInvalidated('watcher');
        }
      }
    `],
    ['captured guard formatting', `
      export class CapturedGuardHandler {
        handle(payload: Payload): void {
          const currentIdentity =
            captureCurrentProjectEventIdentity(
              payload.projectInstanceId,
            );
          if (!currentIdentity) {
            return;
          }
          projectPublicationCoordinator.submit(payload.result);
        }
      }
    `],
    ['fake effect text', `
      export class FakeEffectTextHandler {
        handle(payload: Payload): void {
          const documentation = 'useGraphDataStore.getState()';
          // getPendingMutation(payload.operationId);
          if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
          notifyIndexInvalidated('watcher');
        }
      }
    `],
  ])('accepts structurally guarded handler with %s', (_, source) => {
    expect(eventHandlerIdentityGuardViolations('guarded.ts', source)).toEqual([]);
  });

  it.each([
    ['guard text in a comment', `
      export class CommentGuardHandler {
        handle(payload: Payload): void {
          // if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
          getPendingMutation(payload.operationId);
        }
      }
    `],
    ['guard text in a string', `
      export class StringGuardHandler {
        handle(payload: Payload): void {
          const documentation = 'if (!isCurrentProjectEvent(payload.projectInstanceId)) return;';
          useResourceStore.getState();
        }
      }
    `],
    ['mismatched captured identifier', `
      export class MismatchedIdentityHandler {
        handle(payload: Payload): void {
          const capturedIdentity = captureCurrentProjectEventIdentity(payload.projectInstanceId);
          if (!otherIdentity) return;
          projectPublicationCoordinator.submit(payload.result);
        }
      }
    `],
    ['real effect before direct guard', `
      export class EarlyDirectEffectHandler {
        handle(payload: Payload): void {
          notifyIndexInvalidated('watcher');
          if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
        }
      }
    `],
    ['real effect before captured guard', `
      export class EarlyCapturedEffectHandler {
        handle(payload: Payload): void {
          useGraphDataStore.getState();
          const identity = captureCurrentProjectEventIdentity(payload.projectInstanceId);
          if (!identity) return;
        }
      }
    `],
  ])('rejects handler with %s', (_, source) => {
    const className = /class (\w+)/.exec(source)?.[1];
    expect(eventHandlerIdentityGuardViolations('unguarded.ts', source)).toEqual([
      `unguarded.ts: ${className}`,
    ]);
  });

  it('rejects stale events before correlation or store access', () => {
    const eventFiles = [
      'src/features/core/sync/handlers/ResourceEventHandler.ts',
      'src/features/core/sync/handlers/ProjectMutationEventHandler.ts',
    ];
    const offenders = eventFiles.flatMap((path) => eventHandlerIdentityGuardViolations(
      path,
      readFileSync(resolve(path), 'utf8'),
    ));

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

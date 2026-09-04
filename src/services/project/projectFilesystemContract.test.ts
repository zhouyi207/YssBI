import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";
import * as ts from "typescript/unstable/ast";
import type { Checker, Project, Symbol as TypeScriptSymbol } from "typescript/unstable/sync";
import { describe, expect, it } from "vitest";
import type { ArchitectureSource } from "@/tests/helpers/moduleDependencyAudit";
import { isTauriInvokeCall } from "@/tests/helpers/tauriInvokeAudit";
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from "@/tests/helpers/typescriptAudit";

const sourceRoot = resolve("src");

function isProductionSourcePath(path: string): boolean {
  const normalizedPath = path.replace(/\\/g, "/");
  return (
    [".ts", ".tsx"].includes(extname(normalizedPath)) &&
    !/\.(?:test|spec)\.[^/]+$/.test(normalizedPath) &&
    !/(?:^|\/)__tests__(?:\/|$)/.test(normalizedPath)
  );
}

function productionSources(directory = sourceRoot): Array<{ path: string; source: string }> {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionSources(path);
    if (!isProductionSourcePath(path)) return [];
    return [
      {
        path: relative(resolve("."), path).replace(/\\/g, "/"),
        source: readFileSync(path, "utf8"),
      },
    ];
  });
}

const lifecycleOwnedNodeCommandIdentityFields = {
  transform_graph_draft: "projectInstanceId",
  compile_graph_draft: "projectInstanceId",
  update_function_signature: "projectInstanceId",
  hydrate_editor_graph: "projectInstanceId",
  export_graph_subgraph: "projectInstanceId",
  get_project_history_status: "projectInstanceId",
  undo_graph_document: "projectInstanceId",
  redo_graph_document: "projectInstanceId",
  execute_graph_document: "projectInstanceId",
} as const;

const projectDatabaseIdentityFields = {
  load_database: "projectInstanceId",
  delete_database: "projectInstanceId",
  rename_database: "projectInstanceId",
  get_database_meta: "projectInstanceId",
  get_database_rows: "projectInstanceId",
  get_column_stats: "projectInstanceId",
  get_column_distribution: "projectInstanceId",
  get_dataset_overview: "projectInstanceId",
  edit_cell: "projectInstanceId",
  add_row: "projectInstanceId",
  delete_rows: "projectInstanceId",
  add_column: "projectInstanceId",
  delete_column: "projectInstanceId",
  cast_column: "projectInstanceId",
  rename_column: "projectInstanceId",
  undo_edit: "projectInstanceId",
  redo_edit: "projectInstanceId",
  save_database_changes: "projectInstanceId",
  export_database: "projectInstanceId",
  get_edit_state: "projectInstanceId",
} as const;

const activeProjectCommandIdentityFields = {
  get_localized_node_catalog: "projectInstanceId",
  get_compatible_node_catalog: "projectInstanceId",
  get_project_databases_variables: "projectInstanceId",
  get_project_path: "projectInstanceId",
  get_project_index: "projectInstanceId",
  get_project_resource_path: "projectInstanceId",
  load_project_graph: "projectInstanceId",
  unload_project_graph: "projectInstanceId",
  save_project_graph: "projectInstanceId",
  flush_project: "projectInstanceId",
  save_project_as: "projectInstanceId",
  delete_registered_project_files: "expectedActiveProjectInstanceId",
  create_event: "projectInstanceId",
  create_function: "projectInstanceId",
  duplicate_graph: "projectInstanceId",
  remove_graph: "projectInstanceId",
  rename_graph_resource: "projectInstanceId",
  create_variable: "projectInstanceId",
  get_variable: "projectInstanceId",
  update_variable: "projectInstanceId",
  delete_variable: "projectInstanceId",
  create_chart: "projectInstanceId",
  duplicate_chart: "projectInstanceId",
  load_chart: "projectInstanceId",
  save_chart: "projectInstanceId",
  rename_chart_resource: "projectInstanceId",
  remove_chart: "projectInstanceId",
  get_plot_column_pair: "projectInstanceId",
  ...projectDatabaseIdentityFields,
  ...lifecycleOwnedNodeCommandIdentityFields,
} as const;

const activeProjectCommands = Object.keys(activeProjectCommandIdentityFields) as Array<
  keyof typeof activeProjectCommandIdentityFields
>;

const bootstrapCommandExemptions = [
  "get_current_project_activation",
  "default_project_parent_directory",
  "validate_new_project_path",
  "list_registered_projects",
  "scan_projects_in_directory",
  "cancel_project_picker_task",
  "cleanup_invalid_registered_projects",
  "register_project",
  "remove_registered_project",
  "toggle_registered_project_favorite",
  "get_project_registry_path",
  "create_project",
  "new_project",
  "load_project",
] as const;

const globalCommandExemptions = [
  "get_window_states",
  "get_window_state",
  "save_window_state",
  "get_application_settings",
  "update_application_settings",
  "list_sqlite_tables",
  "list_sql_tables",
  "list_excel_sheets",
  "hypothesis_test",
  "parse_at_values",
  "compute_acf_pacf",
  "compute_serial_tests",
  "compute_panel_did_fake_group_ri",
  "parse_bayes_expression",
  "validate_bayes_model",
  "get_julia_runtime_status",
  "get_julia_worker_status",
  "install_julia_runtime",
  "submit_frontend_diagnostics",
  "subscribe_diagnostics",
  "unsubscribe_diagnostics",
] as const;

const processGlobalAllocatorCommandExemptions = [
  // Task 9 preview generations are process-global, checked, and non-reusable.
  "allocate_pin_preview_generation",
] as const;

const capabilityCommandExemptions = [
  "cancel_graph_run",
  "get_result_descriptor",
  "get_result_value",
  "get_result_page",
  "get_pin_result_history",

  "submit_bayes_inference",
  "get_bayes_inference_status",
  "cancel_bayes_inference",
  "read_bayes_inference_result",
  "clear_bayes_inference_task",
  "export_bayes_artifact_csv",
  "read_bayes_posterior_samples",
  "read_bayes_trace_plot_data",
  "read_bayes_density_plot_data",
  "read_bayes_autocorrelation_data",
  "read_bayes_posterior_predictive",

  "get_harness_runtime_status",
  "configure_harness_provider",
  "create_harness_session",
  "subscribe_harness_events",
  "unsubscribe_harness_events",
  "submit_harness_turn",
  "cancel_harness_turn",
  "close_harness_session",
  "list_harness_memory",
  "delete_harness_memory",
  "plan_dataset_quality_review",
  "advance_harness_workflow",
  "pause_harness_workflow",
  "resume_harness_workflow",
  "cancel_harness_workflow",
] as const;

const identityExemptCommands = [
  ...bootstrapCommandExemptions,
  ...globalCommandExemptions,
  ...processGlobalAllocatorCommandExemptions,
  ...capabilityCommandExemptions,
] as const;

function registeredTauriCommands(source: string): string[] {
  const handler = source.match(/tauri::generate_handler!\[([\s\S]*?)\]/)?.[1] ?? "";
  return handler
    .replace(/\/\/.*$/gm, "")
    .split(",")
    .map((command) => command.trim())
    .filter(Boolean);
}

interface ServiceInvoke {
  command: string;
  path: string;
  payloadFields: string[] | null;
}

function isStringLiteralLike(
  node: ts.Node,
): node is ts.StringLiteral | ts.NoSubstitutionTemplateLiteral {
  return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node);
}

function objectLiteralFields(node: ts.Expression | undefined): string[] | null {
  if (!node || !ts.isObjectLiteralExpression(node)) return null;
  return node.properties.flatMap((property) => {
    if (ts.isShorthandPropertyAssignment(property) && ts.isIdentifier(property.name)) {
      return [property.name.text];
    }
    if (!ts.isPropertyAssignment(property)) return [];
    return ts.isIdentifier(property.name) || isStringLiteralLike(property.name)
      ? [property.name.text]
      : [];
  });
}

function importDeclarationOf(
  binding: ts.ImportSpecifier | ts.NamespaceImport,
): ts.ImportDeclaration | null {
  const importClause = ts.isImportSpecifier(binding) ? binding.parent.parent : binding.parent;
  return ts.isImportClause(importClause) && ts.isImportDeclaration(importClause.parent)
    ? importClause.parent
    : null;
}

function importModuleSpecifier(binding: ts.ImportSpecifier | ts.NamespaceImport): string | null {
  const declaration = importDeclarationOf(binding);
  return declaration !== null && ts.isStringLiteral(declaration.moduleSpecifier)
    ? declaration.moduleSpecifier.text
    : null;
}

function isIpcHelperImport(binding: ts.ImportSpecifier): boolean {
  return importModuleSpecifier(binding) === "@/services/ipc";
}

function symbolHasInvokeCommandImport(
  symbol: TypeScriptSymbol | undefined,
  project: Project,
): boolean {
  return (
    symbol?.declarations.some((handle) => {
      const declaration = handle.resolve(project);
      return (
        declaration !== undefined &&
        ts.isImportSpecifier(declaration) &&
        (declaration.propertyName ?? declaration.name).text === "invokeCommand" &&
        isIpcHelperImport(declaration)
      );
    }) ?? false
  );
}

function isServiceCommandInvokeCall(
  expression: ts.Expression,
  checker: Checker,
  project: Project,
): boolean {
  return (
    isTauriInvokeCall(expression, checker, project) ||
    (ts.isIdentifier(expression) &&
      symbolHasInvokeCommandImport(checker.getSymbolAtLocation(expression), project))
  );
}

function sourceFileInvokes(
  path: string,
  sourceFile: ts.SourceFile,
  project: Project,
): ServiceInvoke[] {
  const invokes: ServiceInvoke[] = [];
  const checker = project.checker;
  const visit = (node: ts.Node): void => {
    if (
      ts.isCallExpression(node) &&
      isServiceCommandInvokeCall(node.expression, checker, project) &&
      node.arguments.length > 0 &&
      isStringLiteralLike(node.arguments[0])
    ) {
      invokes.push({
        command: node.arguments[0].text,
        path,
        payloadFields: objectLiteralFields(node.arguments[1]),
      });
    }
    node.forEachChild(visit);
  };
  visit(sourceFile);
  return invokes;
}

function serviceInvokes(sources: readonly ArchitectureSource[]): ServiceInvoke[] {
  const sourceMap = new Map(sources.map(({ path, source }) => [path, source]));
  return withIsolatedTypeScriptProject(sourceMap, (context) =>
    sources.flatMap(({ path }) =>
      sourceFileInvokes(path, context.sourceFile(path), context.project),
    ),
  );
}

function productionServiceInvokes(sources: readonly ArchitectureSource[]): ServiceInvoke[] {
  return withProductionTypeScriptProject((context) =>
    sources.flatMap(({ path }) =>
      sourceFileInvokes(path, context.sourceFile(path), context.project),
    ),
  );
}

function commandClassificationViolations(
  registered: readonly string[],
  identityRequired: readonly string[],
  exemptions: readonly string[],
): { duplicates: string[]; unclassified: string[]; staleClassifications: string[] } {
  const classified = [...identityRequired, ...exemptions];
  return {
    duplicates: classified.filter((command, index) => classified.indexOf(command) !== index),
    unclassified: registered.filter((command) => !classified.includes(command)),
    staleClassifications: classified.filter((command) => !registered.includes(command)),
  };
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
        : [`${path}: ${command} missing ${identityField}`],
    );
  });
}

function unwrapParentheses(expression: ts.Expression): ts.Expression {
  let current = expression;
  while (ts.isParenthesizedExpression(current)) current = current.expression;
  return current;
}

function negatedExpression(expression: ts.Expression): ts.Expression | null {
  const unwrapped = unwrapParentheses(expression);
  return ts.isPrefixUnaryExpression(unwrapped) &&
    unwrapped.operator === ts.SyntaxKind.ExclamationToken
    ? unwrapParentheses(unwrapped.operand)
    : null;
}

function returnsImmediately(statement: ts.Statement): boolean {
  if (ts.isReturnStatement(statement)) return true;
  return (
    ts.isBlock(statement) &&
    statement.statements.length === 1 &&
    ts.isReturnStatement(statement.statements[0])
  );
}

function capturedIdentityNames(statement: ts.Statement): string[] {
  if (!ts.isVariableStatement(statement)) return [];
  return statement.declarationList.declarations.flatMap((declaration) => {
    if (!ts.isIdentifier(declaration.name) || !declaration.initializer) return [];
    const initializer = unwrapParentheses(declaration.initializer);
    return ts.isCallExpression(initializer) &&
      ts.isIdentifier(initializer.expression) &&
      initializer.expression.text === "captureCurrentProjectEventIdentity"
      ? [declaration.name.text]
      : [];
  });
}

function isIdentityGuard(
  statement: ts.Statement,
  capturedIdentities: ReadonlySet<string>,
): boolean {
  if (
    !ts.isIfStatement(statement) ||
    statement.elseStatement ||
    !returnsImmediately(statement.thenStatement)
  )
    return false;
  const guardedExpression = negatedExpression(statement.expression);
  if (guardedExpression === null) return false;
  if (ts.isIdentifier(guardedExpression)) {
    return capturedIdentities.has(guardedExpression.text);
  }
  return (
    ts.isCallExpression(guardedExpression) &&
    ts.isIdentifier(guardedExpression.expression) &&
    guardedExpression.expression.text === "isCurrentProjectEvent"
  );
}

function isForbiddenHandlerEffect(node: ts.CallExpression): boolean {
  if (ts.isIdentifier(node.expression)) {
    return (
      node.expression.text === "getPendingMutation" ||
      node.expression.text === "notifyIndexInvalidated"
    );
  }
  if (!ts.isPropertyAccessExpression(node.expression)) return false;
  const owner = node.expression.expression;
  const member = node.expression.name.text;
  return (
    ts.isIdentifier(owner) &&
    ((owner.text === "useGraphProjectionStore" && member === "getState") ||
      (owner.text === "useResourceStore" && member === "getState") ||
      (owner.text === "projectPublicationCoordinator" && member === "submit"))
  );
}

function containsForbiddenHandlerEffect(statement: ts.Statement): boolean {
  let forbidden = false;
  const visit = (node: ts.Node): void => {
    if (forbidden || (node !== statement && ts.isFunctionLikeDeclaration(node))) return;
    if (ts.isCallExpression(node) && isForbiddenHandlerEffect(node)) {
      forbidden = true;
      return;
    }
    node.forEachChild(visit);
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

function sourceFileEventHandlerIdentityGuardViolations(
  path: string,
  sourceFile: ts.SourceFile,
): string[] {
  const violations: string[] = [];
  sourceFile.forEachChild((node) => {
    if (!ts.isClassDeclaration(node) || !node.name) return;
    const handle = node.members.find(
      (member): member is ts.MethodDeclaration =>
        ts.isMethodDeclaration(member) &&
        ts.isIdentifier(member.name) &&
        member.name.text === "handle",
    );
    if (!handle?.body || !handleHasIdentityGuardBeforeEffects(handle.body)) {
      violations.push(`${path}: ${node.name.text}`);
    }
  });
  return violations;
}

function eventHandlerIdentityGuardViolations(path: string, source: string): string[] {
  return withIsolatedTypeScriptProject({ [path]: source }, (context) =>
    sourceFileEventHandlerIdentityGuardViolations(path, context.sourceFile(path)),
  );
}

const workflowFiles = [
  "src/features/application/project/projectIOStore.ts",
  "src/features/application/graphProjection/graphProjectionLifecycle.ts",
  "src/features/application/editor/graphDocumentUnload.ts",
  "src/features/application/editor/chartDelete.ts",
  "src/features/application/editor/saveAllDirtyGraphs.ts",

  "src/features/application/editor/useProjectOperations.ts",
  "src/features/application/editor/useChartManagement.ts",
  "src/features/application/dataManagement/variableActions.ts",
  "src/features/application/resource/resourceActions.ts",
  "src/features/application/project/useProjectPicker.ts",
] as const;

describe("projectFilesystemContract", () => {
  it("classifies every registered Tauri command without duplicate or stale exemptions", () => {
    const registered = registeredTauriCommands(
      readFileSync(resolve("src-tauri/crates/yss-api/src/lib.rs"), "utf8"),
    );
    const violations = commandClassificationViolations(
      registered,
      activeProjectCommands,
      identityExemptCommands,
    );

    expect(violations).toEqual({
      duplicates: [],
      unclassified: [],
      staleClassifications: [],
    });
  });

  it("extracts invoke payload keys semantically without matching comments or values", () => {
    const invokes = serviceInvokes([
      {
        path: "src/services/project/fixture.ts",
        source: `
        import { invoke } from '@tauri-apps/api/core';
        // invoke('execute_graph_document', { projectInstanceId });
        invoke('execute_graph_document', { other: projectInstanceId });
      `,
      },
    ]);

    expect(invokes).toEqual([
      {
        command: "execute_graph_document",
        path: "src/services/project/fixture.ts",
        payloadFields: ["other"],
      },
    ]);
  });

  it("recognizes aliased and namespace Tauri invoke bindings", () => {
    const path = "src/services/project/boundInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import { invoke as tauriInvoke } from '@tauri-apps/api/core';
      import * as core from '@tauri-apps/api/core';
      tauriInvoke('get_database_rows', { id, offset, limit });
      core.invoke('edit_cell', { projectId: projectInstanceId, id, row, colName, value });
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
        edit_cell: "projectInstanceId",
      }),
    ).toEqual([
      `${path}: get_database_rows missing projectInstanceId`,
      `${path}: edit_cell missing projectInstanceId`,
    ]);
  });

  it("recognizes aliased invokeCommand service bindings", () => {
    const path = "src/services/project/helperInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import { invokeCommand as callBackend } from '@/services/ipc';
      callBackend('get_database_rows', { id, offset, limit });
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
      }),
    ).toEqual([`${path}: get_database_rows missing projectInstanceId`]);
  });

  it("ignores local invoke decoys that are not bound to Tauri core", () => {
    const path = "src/services/project/localInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      function invoke(_command: string, _payload: unknown) {}
      const core = { invoke };
      invoke('get_database_rows', { projectInstanceId, id, offset, limit });
      core.invoke('get_database_rows', { projectInstanceId, id, offset, limit });
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
      }),
    ).toEqual(["missing service invoke: get_database_rows"]);
  });

  it("checks every real Tauri invocation when valid and invalid calls are mixed", () => {
    const path = "src/services/project/mixedInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import { invoke } from '@tauri-apps/api/core';
      invoke('get_database_rows', { projectInstanceId, id, offset, limit });
      invoke('get_database_rows', { projectId: projectInstanceId, id, offset, limit });
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
      }),
    ).toEqual([`${path}: get_database_rows missing projectInstanceId`]);
  });

  it("ignores lexically shadowed named Tauri imports", () => {
    const path = "src/services/project/shadowedNamedInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import { invoke as tauriInvoke } from '@tauri-apps/api/core';
      function decoy(tauriInvoke: (command: string, payload: unknown) => void) {
        tauriInvoke('get_database_rows', { projectInstanceId, id, offset, limit });
      }
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
      }),
    ).toEqual(["missing service invoke: get_database_rows"]);
  });

  it("ignores lexically shadowed Tauri namespace imports", () => {
    const path = "src/services/project/shadowedNamespaceInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import * as core from '@tauri-apps/api/core';
      function decoy(core: { invoke(command: string, payload: unknown): void }) {
        core.invoke('edit_cell', { projectInstanceId, id, row, colName, value });
      }
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        edit_cell: "projectInstanceId",
      }),
    ).toEqual(["missing service invoke: edit_cell"]);
  });

  it("checks real calls while ignoring mixed shadowed named and namespace calls", () => {
    const path = "src/services/project/mixedShadowedInvokeFixture.ts";
    const invokes = serviceInvokes([
      {
        path,
        source: `
      import { invoke as tauriInvoke } from '@tauri-apps/api/core';
      import * as core from '@tauri-apps/api/core';
      tauriInvoke('get_database_rows', { projectInstanceId, id, offset, limit });
      function namedDecoy(tauriInvoke: (command: string, payload: unknown) => void) {
        tauriInvoke('get_database_rows', { id, offset, limit });
      }
      function namespaceDecoy(core: { invoke(command: string, payload: unknown): void }) {
        core.invoke('get_database_rows', { id, offset, limit });
      }
    `,
      },
    ]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_database_rows: "projectInstanceId",
      }),
    ).toEqual([]);
  });

  it("classifies localized catalog reads as active-project identity-required", () => {
    expect(activeProjectCommandIdentityFields).toMatchObject({
      get_localized_node_catalog: "projectInstanceId",
    });
    expect(bootstrapCommandExemptions).not.toContain("get_localized_node_catalog");
  });

  it("classifies compatible catalog reads as active-project identity-required", () => {
    expect(activeProjectCommandIdentityFields).toMatchObject({
      get_compatible_node_catalog: "projectInstanceId",
    });
    expect(bootstrapCommandExemptions).not.toContain("get_compatible_node_catalog");
  });

  it.each([
    [
      "removed",
      "import { invoke } from '@tauri-apps/api/core'; invoke('get_localized_node_catalog', { locale: 'en-US' });",
    ],
    [
      "renamed",
      "import { invoke } from '@tauri-apps/api/core'; invoke('get_localized_node_catalog', { projectId: projectInstanceId, locale: 'en-US' });",
    ],
  ])("detects %s localized catalog payload identity", (_, source) => {
    const path = "src/services/nodeSystem/catalogMutationFixture.ts";
    const invokes = serviceInvokes([{ path, source }]);

    expect(
      activeProjectInvokeIdentityViolations(invokes, {
        get_localized_node_catalog: "projectInstanceId",
      }),
    ).toEqual([`${path}: get_localized_node_catalog missing projectInstanceId`]);
  });

  it.each([
    [
      "removed get_database_rows identity",
      "get_database_rows",
      "import { invoke } from '@tauri-apps/api/core'; invoke('get_database_rows', { id, offset, limit });",
    ],
    [
      "renamed get_database_rows identity",
      "get_database_rows",
      "import { invoke } from '@tauri-apps/api/core'; invoke('get_database_rows', { projectId: projectInstanceId, id, offset, limit });",
    ],
    [
      "removed edit_cell identity",
      "edit_cell",
      "import { invoke } from '@tauri-apps/api/core'; invoke('edit_cell', { id, row, colName, value });",
    ],
    [
      "renamed edit_cell identity",
      "edit_cell",
      "import { invoke } from '@tauri-apps/api/core'; invoke('edit_cell', { projectId: projectInstanceId, id, row, colName, value });",
    ],
  ])("detects %s in database invoke policy", (_label, command, source) => {
    const path = "src/services/database/databaseMutationFixture.ts";
    const violations = activeProjectInvokeIdentityViolations(serviceInvokes([{ path, source }]), {
      [command]: "projectInstanceId",
    });

    expect(violations).toEqual([`${path}: ${command} missing projectInstanceId`]);
  });

  it("detects get_database_meta incorrectly classified as capability-authorized", () => {
    expect(
      commandClassificationViolations(
        ["get_database_meta"],
        ["get_database_meta"],
        ["get_database_meta"],
      ),
    ).toEqual({
      duplicates: ["get_database_meta"],
      unclassified: [],
      staleClassifications: [],
    });
  });

  it("sends the required identity field in every active-project service invoke", () => {
    const invokes = productionServiceInvokes(productionSources(resolve("src/services")));

    expect(activeProjectInvokeIdentityViolations(invokes)).toEqual([]);
  });

  it("rejects stale direct results before any frontend side effect", () => {
    const offenders = workflowFiles.filter((path) => {
      const source = readFileSync(resolve(path), "utf8");
      if (
        !/await\s+(?:ProjectService|GraphService|GraphProjectionService|VariableService|ChartService)\./.test(
          source,
        )
      ) {
        return false;
      }
      const usesIdentityFacade =
        source.includes("captureProjectIdentity") &&
        (source.includes("isCurrentProjectIdentity") ||
          source.includes("assertCurrentProjectIdentity"));
      const usesCommandContextFacade =
        (source.includes("captureProjectCommandContext") ||
          source.includes("captureRevisionedProjectCommandSnapshot")) &&
        (source.includes(".isCurrent()") || source.includes(".assertCurrent()"));
      const usesLifecycleReceiptOwner =
        source.includes("registerPendingProjectLifecycleOperation") &&
        source.includes(".isCurrent()");
      return !usesIdentityFacade && !usesCommandContextFacade && !usesLifecycleReceiptOwner;
    });

    expect(offenders).toEqual([]);
  });

  it.each([
    [
      "direct guard formatting",
      `
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
    `,
    ],
    [
      "captured guard formatting",
      `
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
    `,
    ],
    [
      "fake effect text",
      `
      export class FakeEffectTextHandler {
        handle(payload: Payload): void {
          const documentation = 'useGraphProjectionStore.getState()';
          // getPendingMutation(payload.operationId);
          if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
          notifyIndexInvalidated('watcher');
        }
      }
    `,
    ],
  ])("accepts structurally guarded handler with %s", (_, source) => {
    expect(eventHandlerIdentityGuardViolations("guarded.ts", source)).toEqual([]);
  });

  it.each([
    [
      "guard text in a comment",
      `
      export class CommentGuardHandler {
        handle(payload: Payload): void {
          // if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
          getPendingMutation(payload.operationId);
        }
      }
    `,
    ],
    [
      "guard text in a string",
      `
      export class StringGuardHandler {
        handle(payload: Payload): void {
          const documentation = 'if (!isCurrentProjectEvent(payload.projectInstanceId)) return;';
          useResourceStore.getState();
        }
      }
    `,
    ],
    [
      "mismatched captured identifier",
      `
      export class MismatchedIdentityHandler {
        handle(payload: Payload): void {
          const capturedIdentity = captureCurrentProjectEventIdentity(payload.projectInstanceId);
          if (!otherIdentity) return;
          projectPublicationCoordinator.submit(payload.result);
        }
      }
    `,
    ],
    [
      "real effect before direct guard",
      `
      export class EarlyDirectEffectHandler {
        handle(payload: Payload): void {
          notifyIndexInvalidated('watcher');
          if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
        }
      }
    `,
    ],
    [
      "real effect before captured guard",
      `
      export class EarlyCapturedEffectHandler {
        handle(payload: Payload): void {
          useGraphProjectionStore.getState();
          const identity = captureCurrentProjectEventIdentity(payload.projectInstanceId);
          if (!identity) return;
        }
      }
    `,
    ],
  ])("rejects handler with %s", (_, source) => {
    const className = /class (\w+)/.exec(source)?.[1];
    expect(eventHandlerIdentityGuardViolations("unguarded.ts", source)).toEqual([
      `unguarded.ts: ${className}`,
    ]);
  });

  it("contains no optional projectInstanceId in active-project service contracts", () => {
    const offenders = productionSources(resolve("src/services")).flatMap(({ path, source }) => {
      const matches =
        source.match(
          /projectInstanceId\s*\?\s*:\s*string|projectInstanceId\s*:\s*string\s*\|\s*(?:null|undefined)|projectInstanceId\s*=\s*[^,;)]+/g,
        ) ?? [];
      return matches.map((match) => `${path}: ${match}`);
    });

    const publicationCoordinator = readFileSync(
      resolve("src/features/application/editorMutation/projectPublicationCoordinator.ts"),
      "utf8",
    );

    expect(offenders).toEqual([]);
    const publicationState =
      publicationCoordinator.match(/interface ProjectPublicationState \{[\s\S]*?\n\}/)?.[0] ?? "";
    expect(publicationState).not.toMatch(/^\s*(?:projectInstanceId|epoch|activationRevision):/m);
  });
});

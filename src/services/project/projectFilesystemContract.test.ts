import { readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const sourceRoot = resolve('src');

function productionSources(directory = sourceRoot): Array<{ path: string; source: string }> {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionSources(path);
    if (!['.ts', '.tsx'].includes(extname(path)) || path.endsWith('.test.ts') || path.endsWith('.test.tsx')) {
      return [];
    }
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
        || source.includes('captureGraphSaveCommandContext'))
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
    const event = readFileSync(resolve('src-tauri/src/event/event_project.rs'), 'utf8');

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

    const identityFacade = readFileSync(resolve('src/services/project/projectIdentity.ts'), 'utf8');
    const projectIOStore = readFileSync(resolve('src/features/core/dataStore/projectIOStore.ts'), 'utf8');

    expect(offenders).toEqual([]);
    expect(identityFacade).toContain('projectPublicationCoordinator');
    expect(identityFacade).not.toMatch(/(?:let|const)\s+\w*epoch\b/i);
    expect(projectIOStore).not.toMatch(/^\s*epoch\s*:/m);
  });
});

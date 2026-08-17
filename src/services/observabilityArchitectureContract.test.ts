import { readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

function productionFiles(directory: string, extensions: readonly string[]): string[] {
  const root = resolve(directory);
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return productionFiles(path, extensions);
    const normalized = relative(resolve('.'), path).replace(/\\/g, '/');
    if (!extensions.includes(extname(path)) || /\.(?:test|spec)\.[^/]+$/.test(normalized)) {
      return [];
    }
    return [normalized];
  });
}

function matchingFiles(
  paths: readonly string[],
  pattern: RegExp,
): string[] {
  return paths.filter((path) => pattern.test(readFileSync(resolve(path), 'utf8')));
}

describe('observability architecture contract', () => {
  it('keeps removed logging stacks and disk pagination out of Rust production code', () => {
    const rustFiles = productionFiles('src-tauri/src', ['.rs']);
    const forbidden = [
      /\btauri_plugin_log\b/,
      /\bLogManager\b/,
      /\bcommand_log\b/,
      /\bget_logs\b/,
      /\bget_log_count\b/,
      /["']log-message["']/,
    ];
    const offenders = forbidden.flatMap((pattern) => matchingFiles(rustFiles, pattern));
    const cargo = readFileSync(resolve('src-tauri/Cargo.toml'), 'utf8');

    expect([...new Set(offenders)]).toEqual([]);
    expect(cargo).not.toMatch(/tauri-plugin-log|tracing-appender/);
  });

  it('keeps other successful and asynchronous status payloads free of failure prose', () => {
    const databaseSource = readFileSync(resolve('src-tauri/src/schema/database.rs'), 'utf8');
    const databaseDto = databaseSource.match(/pub struct DatabaseDeclDTO\s*\{([\s\S]*?)\n\}/)?.[1];
    const didSource = readFileSync(resolve('src-tauri/src/sci/models/panel_did.rs'), 'utf8');
    const didBlock = didSource.match(/pub struct DidPlaceboFakeGroupBlock\s*\{([\s\S]*?)\n\}/)?.[1];
    const worksheetType = readFileSync(resolve('src/shared/types/domain/worksheet.ts'), 'utf8');
    const resultState = readFileSync(resolve('src/features/core/resultSource/types.ts'), 'utf8');
    const catalogState = readFileSync(resolve('src/features/core/nodeCatalog/nodeCatalogStore.ts'), 'utf8');
    const projectState = readFileSync(resolve('src/features/core/dataStore/projectIOStore.ts'), 'utf8');
    const compilerDiagnostics = readFileSync(
      resolve('src-tauri/src/node_system/compiler/diagnostics.rs'),
      'utf8',
    );
    const resourceResolutionDiagnostic = compilerDiagnostics.match(
      /ResourceResolutionFailed\s*\{([^}]*)\}\s*=>\s*\{([\s\S]*?)\n\s*\},/,
    );
    const sqlConnectionModal = readFileSync(resolve('src/shared/ui/SqlConnectionModal.tsx'), 'utf8');
    if (!databaseDto || !didBlock || !resourceResolutionDiagnostic) {
      throw new Error('machine status DTO contract not found');
    }

    expect(databaseDto).toContain('load_failed: bool');
    expect(databaseDto).not.toMatch(/load_error|message|detail|hint/);
    expect(didBlock).toContain('unavailable_code');
    expect(didBlock).not.toMatch(/method_note|message|detail|hint/);
    expect(worksheetType).toMatch(/kind: 'error';\s+code: string;\s+incidentId: string \| null;/);
    expect(worksheetType).not.toMatch(/kind: 'error';\s+message:/);
    expect(resultState).toMatch(/error: ErrorReference \| null/);
    expect(catalogState).toMatch(/error: ErrorReference \| null/);
    expect(projectState).toMatch(/error: ErrorReference \| null/);
    expect(resourceResolutionDiagnostic[1].trim()).toBe('resource_key');
    expect(resourceResolutionDiagnostic[2]).not.toContain('{reason}');
    expect(sqlConnectionModal).not.toMatch(/setError\(String\(/);
  });

  it('keeps status, path validation, and result failures free of backend prose', () => {
    const runtimeStatus = readFileSync(resolve('src-tauri/src/julia/mod.rs'), 'utf8')
      .match(/pub struct JuliaRuntimeStatus\s*\{([\s\S]*?)\n\}/)?.[1];
    const workerStatus = readFileSync(resolve('src-tauri/src/julia/worker.rs'), 'utf8')
      .match(/pub struct JuliaWorkerStatus\s*\{([\s\S]*?)\n\}/)?.[1];
    const resultFailure = readFileSync(resolve('src-tauri/src/commands/node_system_execution_dto.rs'), 'utf8')
      .match(/pub struct ResultFailureDto\s*\{([\s\S]*?)\n\}/)?.[1];
    const pathCommand = readFileSync(resolve('src-tauri/src/commands/command_project/path.rs'), 'utf8');
    if (!runtimeStatus || !workerStatus || !resultFailure) throw new Error('safe status DTO contract not found');

    const fields = (body: string) => [...body.matchAll(/^\s*pub\s+(\w+)\s*:/gm)].map(match => match[1]);
    expect(fields(runtimeStatus)).toEqual(['state', 'version', 'install_dir']);
    expect(fields(workerStatus)).toEqual(['runtime_state', 'environment_state', 'process_state', 'project_dir']);
    expect([...resultFailure.matchAll(/^\s*(\w+)\s*:/gm)].map(match => match[1]))
      .toEqual(['code', 'cause', 'upstream_result_ids']);
    expect(pathCommand).toMatch(/validate_new_project_path\(path: String\) -> Result<\(\), CommandError>/);
    expect(pathCommand).not.toMatch(/ProjectPathValidation/);
  });

  it('keeps business application workflows independent from diagnostic storage', () => {
    const applicationFiles = productionFiles('src/features/application', ['.ts', '.tsx'])
      .filter((path) => !path.startsWith('src/features/application/log/'));
    const diagnosticImports = /(?:@\/features\/core\/log|@\/services\/log|@\/shared\/types\/dto\/diagnostics)/;

    expect(matchingFiles(applicationFiles, diagnosticImports)).toEqual([]);
  });

  it('has no raw tracing formatter or frontend mirror bypassing the sanitizer', () => {
    const runtime = readFileSync(resolve('src-tauri/src/diagnostics/runtime.rs'), 'utf8');
    const dispatcher = readFileSync(resolve('src-tauri/src/diagnostics/dispatcher.rs'), 'utf8');

    expect(runtime).not.toContain('tracing_subscriber::fmt');
    expect(runtime).not.toContain('emit_frontend_to_tracing');
    expect(runtime).toContain('create_console_record_sink');
    expect(runtime).toContain('create_file_record_sink');
    expect(dispatcher).toContain('record.sanitized()');
    expect(dispatcher).toContain('BoundedWorker');
  });
});

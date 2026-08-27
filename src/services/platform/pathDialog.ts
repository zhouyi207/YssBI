import { open, save } from '@tauri-apps/plugin-dialog';
import type { PlatformFailure, PlatformOperation, PlatformOutcome } from './platformTypes';

export interface OpenPathDialogOptions {
  readonly directory?: boolean;
  readonly multiple?: boolean;
  readonly title?: string;
  readonly defaultPath?: string;
  readonly filters?: readonly {
    readonly name: string;
    readonly extensions: readonly string[];
  }[];
}

export type SavePathDialogOptions = Omit<OpenPathDialogOptions, 'directory' | 'multiple'>;

function operationFailure(operation: PlatformOperation): PlatformFailure {
  return { operation, code: 'operationFailed' };
}

function invalidResult(operation: PlatformOperation): PlatformFailure {
  return { operation, code: 'invalidResult', resultKind: 'pathSelection' };
}

function filters(
  value: OpenPathDialogOptions['filters'],
): { name: string; extensions: string[] }[] | undefined {
  return value?.map((filter) => ({
    name: filter.name,
    extensions: [...filter.extensions],
  }));
}

export async function openPathDialog(
  options: OpenPathDialogOptions,
): Promise<PlatformOutcome<string | string[] | null>> {
  try {
    const value = await open({
      directory: options.directory,
      multiple: options.multiple,
      title: options.title,
      defaultPath: options.defaultPath,
      filters: filters(options.filters),
    });
    if (!options.multiple && Array.isArray(value)) {
      return { ok: false, failure: invalidResult('openPathDialog') };
    }
    return { ok: true, value };
  } catch {
    return { ok: false, failure: operationFailure('openPathDialog') };
  }
}

export async function savePathDialog(
  options: SavePathDialogOptions,
): Promise<PlatformOutcome<string | null>> {
  try {
    return {
      ok: true,
      value: await save({
        title: options.title,
        defaultPath: options.defaultPath,
        filters: filters(options.filters),
      }),
    };
  } catch {
    return { ok: false, failure: operationFailure('savePathDialog') };
  }
}

export async function selectDatabaseExportPath(): Promise<PlatformOutcome<string | null>> {
  return savePathDialog({
    title: 'Export Data',
    filters: [
      { name: 'CSV', extensions: ['csv'] },
      { name: 'Parquet', extensions: ['parquet'] },
    ],
  });
}

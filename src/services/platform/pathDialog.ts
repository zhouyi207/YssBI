import type { PlatformOutcome } from './platformTypes';
import { save } from '@tauri-apps/plugin-dialog';

export interface PathDialogAdapter {
  readonly open: () => Promise<PlatformOutcome<string | null>>;
  readonly save: () => Promise<PlatformOutcome<string | null>>;
}

export async function selectDatabaseExportPath(): Promise<PlatformOutcome<string | null>> {
  try {
    return {
      ok: true,
      value: await save({
        title: 'Export Data',
        filters: [
          { name: 'CSV', extensions: ['csv'] },
          { name: 'Parquet', extensions: ['parquet'] },
        ],
      }),
    };
  } catch {
    return { ok: false, failure: { code: 'rejected' } };
  }
}

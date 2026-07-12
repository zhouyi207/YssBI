import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DATA_STORE_DIR = dirname(fileURLToPath(import.meta.url));

/** Zustand hook 名称 — 须在对应 lifecycle 模块中显式 import */
export const PROJECT_SNAPSHOT_STORE_HOOKS = ['useGraphDataStore', 'useResourceStore'] as const;

export const PROJECT_RESET_STORE_HOOKS = [
  'useLayoutStore',
  'useViewportStore',
  'useHistoryStore',
  'useEditStateStore',
  'useColumnStatsStore',
  'useColumnDistributionStore',
  'useDatasetOverviewStore',
  'useWorksheetStore',
  'useResourceStore',
  'useDocumentStateStore',
  'useGraphMetaStore',
] as const;

export const PROJECT_IO_DIRECT_STORE_HOOKS = [
  'useVariableStore',
  'useDatabaseStore',
  'useGraphDataStore',
  'useWorksheetStore',
] as const;

type StoreHookName =
  | (typeof PROJECT_SNAPSHOT_STORE_HOOKS)[number]
  | (typeof PROJECT_RESET_STORE_HOOKS)[number]
  | (typeof PROJECT_IO_DIRECT_STORE_HOOKS)[number];

const LIFECYCLE_MODULES: Array<{ file: string; hooks: readonly StoreHookName[] }> = [
  { file: 'projectSnapshotBridge.ts', hooks: PROJECT_SNAPSHOT_STORE_HOOKS },
  { file: 'projectClientReset.ts', hooks: PROJECT_RESET_STORE_HOOKS },
  {
    file: 'projectIOStore.ts',
    hooks: PROJECT_IO_DIRECT_STORE_HOOKS,
  },
];

function readModuleSource(filename: string): string {
  return readFileSync(join(DATA_STORE_DIR, filename), 'utf8');
}

function importsHook(source: string, hook: string): boolean {
  return new RegExp(`import\\s*\\{[^}]*\\b${hook}\\b`).test(source);
}

/** Vitest：禁止 lifecycle 模块隐式依赖未 import 的 store hook */
export function auditProjectStoreImports(): Array<{ file: string; missing: string[] }> {
  const failures: Array<{ file: string; missing: string[] }> = [];
  for (const { file, hooks } of LIFECYCLE_MODULES) {
    const source = readModuleSource(file);
    const missing = hooks.filter((hook) => !importsHook(source, hook));
    if (missing.length > 0) {
      failures.push({ file, missing });
    }
  }
  return failures;
}

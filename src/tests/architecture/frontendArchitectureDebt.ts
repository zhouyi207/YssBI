import { existsSync } from 'node:fs';

import type {
  FrontendDebtCountMismatch,
  FrontendDebtDeclarationError,
  FrontendDebtEntry,
  FrontendDebtKey,
  FrontendDebtMismatch,
  FrontendFinding,
} from './frontendArchitectureModel';

export type { FrontendDebtEntry } from './frontendArchitectureModel';

export const APPROVED_FRONTEND_MIGRATION_SPECS = [
  'docs/architecture/RUST_BACKEND_ADAPTER_BOUNDARIES.md',
  'docs/architecture/PROJECT_GRAPH_OWNERSHIP_BOUNDARIES.md',
  'docs/architecture/EXECUTION_RUNTIME_BOUNDARIES.md',
  'docs/architecture/PRESENTATION_COMMAND_BOUNDARIES.md',
  'docs/architecture/FRONTEND_APPLICATION_BOUNDARIES.md',
] as const;

/** The architecture cutover has no unresolved frontend debt entries. */
export const FRONTEND_ARCHITECTURE_DEBT: readonly FrontendDebtEntry[] = [];

function keyOf(value: FrontendDebtKey): FrontendDebtKey {
  return {
    ruleId: value.ruleId,
    repositoryRelativeSourceFile: value.repositoryRelativeSourceFile,
    fullyQualifiedOwner: value.fullyQualifiedOwner,
    dependencyKind: value.dependencyKind,
    canonicalOriginTarget: value.canonicalOriginTarget,
  };
}

function keyIdentity(value: FrontendDebtKey): string {
  return JSON.stringify([
    value.ruleId,
    value.repositoryRelativeSourceFile,
    value.fullyQualifiedOwner,
    value.dependencyKind,
    value.canonicalOriginTarget,
  ]);
}

function mismatchSort(left: FrontendDebtCountMismatch, right: FrontendDebtCountMismatch): number {
  return keyIdentity(left).localeCompare(keyIdentity(right));
}

function validateDeclaredDebt(
  declared: readonly FrontendDebtEntry[],
): {
  readonly valid: ReadonlyMap<string, FrontendDebtEntry>;
  readonly errors: readonly FrontendDebtDeclarationError[];
} {
  const valid = new Map<string, FrontendDebtEntry>();
  const errors: FrontendDebtDeclarationError[] = [];
  const approvedSpecs = new Set<string>(APPROVED_FRONTEND_MIGRATION_SPECS);
  const seen = new Set<string>();
  for (const entry of declared) {
    const key = keyOf(entry);
    const identity = keyIdentity(key);
    let isValid = true;
    if (seen.has(identity)) {
      errors.push({ kind: 'duplicate-frontend-debt-key', key });
      isValid = false;
    }
    if (!Number.isSafeInteger(entry.expectedOccurrences) || entry.expectedOccurrences <= 0) {
      errors.push({
        kind: 'invalid-frontend-debt-count',
        key,
        expectedOccurrences: entry.expectedOccurrences,
      });
      isValid = false;
    }
    if (!approvedSpecs.has(entry.owningMigrationSpec) || !existsSync(entry.owningMigrationSpec)) {
      errors.push({
        kind: 'invalid-frontend-debt-owning-spec',
        key,
        owningMigrationSpec: entry.owningMigrationSpec,
      });
      isValid = false;
    }
    if (isValid) valid.set(identity, entry);
    seen.add(identity);
  }
  return {
    valid,
    errors: errors.sort((left, right) => (
      left.kind + '\u0000' + keyIdentity(left.key)
    ).localeCompare(
      right.kind + '\u0000' + keyIdentity(right.key),
    )),
  };
}

export function compareExactFrontendDebt(
  actual: readonly FrontendFinding[],
  declared: readonly FrontendDebtEntry[],
): FrontendDebtMismatch {
  const actualCounts = new Map<string, { readonly key: FrontendDebtKey; count: number }>();
  for (const finding of actual) {
    const key = keyOf(finding);
    const identity = keyIdentity(key);
    const count = actualCounts.get(identity);
    if (count) count.count += 1;
    else actualCounts.set(identity, { key, count: 1 });
  }
  const validated = validateDeclaredDebt(declared);
  const identities = new Set([...actualCounts.keys(), ...validated.valid.keys()]);
  const newOrIncreased: FrontendDebtCountMismatch[] = [];
  const staleOrDecreased: FrontendDebtCountMismatch[] = [];
  for (const identity of identities) {
    const actualCount = actualCounts.get(identity);
    const declaredEntry = validated.valid.get(identity);
    const actualOccurrences = actualCount?.count ?? 0;
    const expectedOccurrences = declaredEntry?.expectedOccurrences ?? 0;
    if (actualOccurrences === expectedOccurrences) continue;
    const key = actualCount?.key ?? keyOf(declaredEntry!);
    const mismatch: FrontendDebtCountMismatch = {
      ...key,
      actualOccurrences,
      expectedOccurrences,
      owningMigrationSpec: declaredEntry?.owningMigrationSpec ?? null,
    };
    if (actualOccurrences > expectedOccurrences) newOrIncreased.push(mismatch);
    else staleOrDecreased.push(mismatch);
  }
  return {
    newOrIncreased: newOrIncreased.sort(mismatchSort),
    staleOrDecreased: staleOrDecreased.sort(mismatchSort),
    errors: validated.errors,
  };
}

import { withIsolatedTypeScriptProject } from '@/tests/helpers/typescriptAudit';
import { describe, expect, it } from 'vitest';
import {
  auditFrontendStateAuthority,
  discoverFrontendStateAuthorityMembers,
  type FrontendStateAuthorityMember,
} from './frontendStateAuthority';

const field = (overrides: Partial<FrontendStateAuthorityMember>): FrontendStateAuthorityMember => ({
  storeModule: 'src/features/core/example/privateStore.ts',
  member: 'snapshot',
  memberKind: 'field',
  authority: 'backend-base',
  writes: ['snapshot'],
  writerModule: '@/features/core/example/publication',
  writerLayer: 'Application',
  readerLayers: ['Application'],
  ...overrides,
});

describe('frontend state authority audit', () => {
  it('fails closed for missing fields and actions', () => {
    const findings = auditFrontendStateAuthority([
      field({ member: 'replaceSnapshot', memberKind: 'action' }),
      field({ member: 'setLocalSelection', authority: 'frontend-ui', writerLayer: 'CoreUi' }),
    ], []);
    expect(findings.map((finding) => finding.member)).toEqual([
      'replaceSnapshot',
      'setLocalSelection',
    ]);
  });

  it('rejects a View writer and canonicalizes computed dirty paths', () => {
    const manifest = [field({ member: 'replaceSnapshot' })];
    const findings = auditFrontendStateAuthority([
      field({ member: 'replaceSnapshot', sourceLayer: 'Views' }),
      field({
        member: 'saveDocument',
        memberKind: 'action',
        authority: 'local-draft',
        writes: ['documents[documentId].dirty'],
        writerModule: '@/features/core/worksheet/reconciliation',
        writerLayer: 'Application',
      }),
    ], manifest);
    expect(findings).toEqual(expect.arrayContaining([
      expect.objectContaining({ ruleId: 'frontend-state-authority-writer', member: 'replaceSnapshot' }),
      expect.objectContaining({
        ruleId: 'frontend-state-authority-missing-member',
        member: 'saveDocument',
        canonicalWritePath: 'documents.*.dirty',
      }),
    ]));
  });

  it('detects delegated action cycles and unresolved delegates', () => {
    const cycleA = field({ member: 'saveA', memberKind: 'action', delegatesTo: 'src/features/core/example/privateStore.ts::action::saveB' });
    const cycleB = field({ member: 'saveB', memberKind: 'action', delegatesTo: 'src/features/core/example/privateStore.ts::action::saveA' });
    const unresolved = field({ member: 'saveC', memberKind: 'action', delegatesTo: 'missing' });
    const findings = auditFrontendStateAuthority([cycleA, cycleB, unresolved], [
      cycleA,
      cycleB,
      unresolved,
    ]);
    expect(findings).toEqual(expect.arrayContaining([
      expect.objectContaining({ ruleId: 'frontend-state-authority-action-cycle' }),
      expect.objectContaining({ ruleId: 'frontend-state-authority-unresolved-delegate' }),
    ]));
  });

  it('discovers manifest members from the project snapshot and fails closed when one is absent', () => {
    const storeModule = 'src/features/core/fixture/authorityStore.ts';
    const sources = [{
      path: storeModule,
      source: `
        import { create } from 'zustand';
        interface FixtureStore {
          projectInstanceId: string | null;
          updateDocument(path: string): void;
        }
        export const useFixtureStore = create<FixtureStore>((set) => ({
          projectInstanceId: null,
          updateDocument: (path) => set({ projectInstanceId: path }),
        }));
      `,
    }];
    const manifest = [
      field({ storeModule, member: 'projectInstanceId' }),
      field({ storeModule, member: 'updateDocument', memberKind: 'action' }),
      field({ storeModule, member: 'missingAction', memberKind: 'action' }),
    ];

    withIsolatedTypeScriptProject(new Map(sources.map(({ path, source }) => [path, source])), (context) => {
      const members = discoverFrontendStateAuthorityMembers(context, sources, manifest);
      expect(members.map(({ member, discovered }) => ({ member, discovered }))).toEqual([
        { member: 'projectInstanceId', discovered: true },
        { member: 'updateDocument', discovered: true },
        { member: 'missingAction', discovered: false },
      ]);
      expect(auditFrontendStateAuthority(members, manifest)).toEqual([
        expect.objectContaining({
          ruleId: 'frontend-state-authority-missing-member',
          storeModule,
          member: 'missingAction',
          memberKind: 'action',
        }),
      ]);
    });
  });
});
